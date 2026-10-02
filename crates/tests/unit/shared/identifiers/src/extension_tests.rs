use systemprompt_identifiers::{DbValue, ExtensionId, ToDbValue};

#[test]
fn extension_id_new_keeps_value() {
    let id = ExtensionId::new("users");
    assert_eq!(id.as_str(), "users");
    assert_eq!(id.to_string(), "users");
}

#[test]
fn extension_id_try_new_and_parse_agree() {
    let a = ExtensionId::try_new("managed_resources").unwrap();
    let b: ExtensionId = "managed_resources".parse().unwrap();
    assert_eq!(a, b);
}

#[test]
fn extension_id_rejects_blank() {
    ExtensionId::try_new("").unwrap_err();
    ExtensionId::try_new("   ").unwrap_err();
    "".parse::<ExtensionId>().unwrap_err();
    serde_json::from_str::<ExtensionId>("\"\"").unwrap_err();
}

#[test]
fn extension_id_serde_transparent() {
    let id = ExtensionId::new("content");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"content\"");
    let back: ExtensionId = serde_json::from_str(&json).unwrap();
    assert_eq!(back, id);
}

#[test]
fn extension_id_to_db_value() {
    let id = ExtensionId::new("ai");
    assert!(matches!(id.to_db_value(), DbValue::String(ref s) if s == "ai"));
}
