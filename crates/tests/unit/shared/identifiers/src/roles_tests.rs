use systemprompt_identifiers::{DbValue, RoleId, ToDbValue};

#[test]
fn role_id_display_format() {
    let id = RoleId::try_new("admin").expect("valid RoleId");
    assert_eq!(format!("{}", id), "admin");
}

#[test]
fn role_id_serde_transparent_json() {
    let id = RoleId::try_new("editor").expect("valid RoleId");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"editor\"");
    let deserialized: RoleId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn role_id_accepts_equal_values_from_str_and_string() {
    let a = RoleId::try_new("admin").expect("valid RoleId");
    let b = RoleId::try_new(String::from("admin")).expect("valid RoleId");
    assert_eq!(a, b);
}

#[test]
fn role_id_try_new_rejects_empty() {
    assert!(RoleId::try_new("").is_err());
    assert!(RoleId::try_new("admin").is_ok());
}

#[test]
fn role_id_into_string() {
    let s: String = RoleId::try_new("admin").expect("valid RoleId").into();
    assert_eq!(s, "admin");
}

#[test]
fn role_id_partial_eq_str() {
    let id = RoleId::try_new("admin").expect("valid RoleId");
    assert!(id == "admin");
    assert!("admin" == id);
}

#[test]
fn role_id_to_db_value_owned_and_ref() {
    let id = RoleId::try_new("admin").expect("valid RoleId");
    assert!(matches!(id.to_db_value(), DbValue::String(ref s) if s == "admin"));
    assert!(matches!((&id).to_db_value(), DbValue::String(ref s) if s == "admin"));
}
