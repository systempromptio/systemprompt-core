use systemprompt_identifiers::{ClientId, ClientType, DbValue, ToDbValue};

#[test]
fn client_type_cimd_for_https_prefix() {
    let id = ClientId::new("https://example.com/mcp");
    assert_eq!(id.client_type(), ClientType::Cimd);
}

#[test]
fn client_type_first_party_for_sp_prefix() {
    let id = ClientId::new("sp_web");
    assert_eq!(id.client_type(), ClientType::FirstParty);
}

#[test]
fn client_type_third_party_for_client_prefix() {
    let id = ClientId::new("client_abc123");
    assert_eq!(id.client_type(), ClientType::ThirdParty);
}

#[test]
fn client_type_system_for_sys_prefix() {
    let id = ClientId::new("sys_scheduler");
    assert_eq!(id.client_type(), ClientType::System);
}

#[test]
fn client_type_unknown_for_unrecognized_prefix() {
    let id = ClientId::new("random-client-id");
    assert_eq!(id.client_type(), ClientType::Unknown);
}

#[test]
fn is_dcr_true_for_first_party() {
    assert!(ClientId::web().is_dcr());
    assert!(ClientId::cli().is_dcr());
    assert!(ClientId::desktop().is_dcr());
}

#[test]
fn is_dcr_true_for_third_party() {
    assert!(ClientId::new("client_third").is_dcr());
}

#[test]
fn is_dcr_false_for_cimd() {
    assert!(!ClientId::new("https://example.com").is_dcr());
}

#[test]
fn is_dcr_false_for_system() {
    assert!(!ClientId::system("worker").is_dcr());
}

#[test]
fn is_dcr_false_for_unknown() {
    assert!(!ClientId::new("arbitrary").is_dcr());
}

#[test]
fn is_cimd_true_for_https_url() {
    assert!(ClientId::new("https://example.com/endpoint").is_cimd());
}

#[test]
fn is_cimd_false_for_http_url() {
    assert!(!ClientId::new("http://example.com").is_cimd());
}

#[test]
fn is_system_true_for_sys_prefix() {
    assert!(ClientId::system("worker").is_system());
}

#[test]
fn is_system_false_for_non_sys() {
    assert!(!ClientId::web().is_system());
}

#[test]
fn factory_methods_produce_correct_values() {
    assert_eq!(ClientId::web().as_str(), "sp_web");
    assert_eq!(ClientId::cli().as_str(), "sp_cli");
    assert_eq!(ClientId::mobile_ios().as_str(), "sp_mobile_ios");
    assert_eq!(ClientId::mobile_android().as_str(), "sp_mobile_android");
    assert_eq!(ClientId::desktop().as_str(), "sp_desktop");
}

#[test]
fn system_factory_formats_with_prefix() {
    let id = ClientId::system("scheduler");
    assert_eq!(id.as_str(), "sys_scheduler");
    assert!(id.is_system());
}

#[test]
fn client_type_as_str_values() {
    assert_eq!(ClientType::Cimd.as_str(), "cimd");
    assert_eq!(ClientType::FirstParty.as_str(), "firstparty");
    assert_eq!(ClientType::ThirdParty.as_str(), "thirdparty");
    assert_eq!(ClientType::System.as_str(), "system");
    assert_eq!(ClientType::Unknown.as_str(), "unknown");
}

#[test]
fn client_type_display_matches_as_str() {
    assert_eq!(format!("{}", ClientType::Cimd), ClientType::Cimd.as_str());
    assert_eq!(
        format!("{}", ClientType::FirstParty),
        ClientType::FirstParty.as_str()
    );
}

#[test]
fn client_type_serde_rename_all_lowercase() {
    let json = serde_json::to_string(&ClientType::FirstParty).unwrap();
    assert_eq!(json, "\"firstparty\"");
    let deserialized: ClientType = serde_json::from_str("\"thirdparty\"").unwrap();
    assert_eq!(deserialized, ClientType::ThirdParty);
}

#[test]
fn client_id_serde_transparent_json() {
    let id = ClientId::web();
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"sp_web\"");
    let deserialized: ClientId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn client_id_from_string_and_str_produce_equal() {
    let from_str: ClientId = "test".into();
    let from_string: ClientId = String::from("test").into();
    assert_eq!(from_str, from_string);
}

#[test]
fn client_id_into_string() {
    let id = ClientId::new("convert-me");
    let s: String = id.into();
    assert_eq!(s, "convert-me");
}

#[test]
fn client_id_partial_eq_str() {
    let id = ClientId::web();
    assert!(id == "sp_web");
    assert!("sp_web" == id);
}

#[test]
fn client_id_to_db_value() {
    let id = ClientId::web();
    let db_val = id.to_db_value();
    assert!(matches!(db_val, DbValue::String(ref s) if s == "sp_web"));
}
