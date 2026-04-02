use systemprompt_identifiers::{UserId, DbValue, ToDbValue};

#[test]
fn anonymous_factory_value() {
    assert_eq!(UserId::anonymous().as_str(), "anonymous");
}

#[test]
fn system_factory_value() {
    assert_eq!(UserId::system().as_str(), "system");
}

#[test]
fn is_system_true_for_system() {
    assert!(UserId::system().is_system());
}

#[test]
fn is_system_false_for_others() {
    assert!(!UserId::new("user-123").is_system());
    assert!(!UserId::anonymous().is_system());
}

#[test]
fn is_anonymous_true_for_anonymous() {
    assert!(UserId::anonymous().is_anonymous());
}

#[test]
fn is_anonymous_false_for_others() {
    assert!(!UserId::new("user-123").is_anonymous());
    assert!(!UserId::system().is_anonymous());
}

#[test]
fn system_and_anonymous_are_distinct() {
    assert_ne!(UserId::system(), UserId::anonymous());
}

#[test]
fn display_format() {
    let id = UserId::new("user-42");
    assert_eq!(format!("{}", id), "user-42");
}

#[test]
fn serde_transparent_json() {
    let id = UserId::new("serde-user");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serde-user\"");
    let deserialized: UserId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn from_string_and_str_produce_equal() {
    let from_str: UserId = "test".into();
    let from_string: UserId = String::from("test").into();
    assert_eq!(from_str, from_string);
}

#[test]
fn into_string_conversion() {
    let id = UserId::new("convert");
    let s: String = id.into();
    assert_eq!(s, "convert");
}

#[test]
fn partial_eq_str() {
    let id = UserId::new("cmp");
    assert!(id == "cmp");
    assert!("cmp" == id);
}

#[test]
fn to_db_value_owned_and_ref() {
    let id = UserId::new("db");
    assert!(matches!(id.to_db_value(), DbValue::String(ref s) if s == "db"));
    assert!(matches!((&id).to_db_value(), DbValue::String(ref s) if s == "db"));
}

#[test]
fn accepts_email_format() {
    let id = UserId::new("user@example.com");
    assert_eq!(id.as_str(), "user@example.com");
}

#[test]
fn accepts_uuid_format() {
    let id = UserId::new("550e8400-e29b-41d4-a716-446655440000");
    assert_eq!(id.as_str(), "550e8400-e29b-41d4-a716-446655440000");
}
