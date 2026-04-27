use systemprompt_identifiers::{DbValue, RoleId, ToDbValue};

#[test]
fn role_id_display_format() {
    let id = RoleId::new("admin");
    assert_eq!(format!("{}", id), "admin");
}

#[test]
fn role_id_serde_transparent_json() {
    let id = RoleId::new("editor");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"editor\"");
    let deserialized: RoleId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn role_id_from_str_and_string_equal() {
    let a: RoleId = "admin".into();
    let b: RoleId = String::from("admin").into();
    assert_eq!(a, b);
}

#[test]
fn role_id_into_string() {
    let s: String = RoleId::new("admin").into();
    assert_eq!(s, "admin");
}

#[test]
fn role_id_partial_eq_str() {
    let id = RoleId::new("admin");
    assert!(id == "admin");
    assert!("admin" == id);
}

#[test]
fn role_id_to_db_value_owned_and_ref() {
    let id = RoleId::new("admin");
    assert!(matches!(id.to_db_value(), DbValue::String(ref s) if s == "admin"));
    assert!(matches!((&id).to_db_value(), DbValue::String(ref s) if s == "admin"));
}
