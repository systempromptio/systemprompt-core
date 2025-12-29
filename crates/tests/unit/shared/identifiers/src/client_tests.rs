//! Unit tests for ClientId and ClientType types.

use std::collections::HashSet;
use systemprompt_identifiers::{ClientId, ClientType, ToDbValue, DbValue};

// ============================================================================
// ClientId Tests
// ============================================================================

#[test]
fn test_client_id_new() {
    let id = ClientId::new("client-123");
    assert_eq!(id.as_str(), "client-123");
}

#[test]
fn test_client_id_web() {
    let id = ClientId::web();
    assert_eq!(id.as_str(), "sp_web");
}

#[test]
fn test_client_id_cli() {
    let id = ClientId::cli();
    assert_eq!(id.as_str(), "sp_cli");
}

#[test]
fn test_client_id_mobile_ios() {
    let id = ClientId::mobile_ios();
    assert_eq!(id.as_str(), "sp_mobile_ios");
}

#[test]
fn test_client_id_mobile_android() {
    let id = ClientId::mobile_android();
    assert_eq!(id.as_str(), "sp_mobile_android");
}

#[test]
fn test_client_id_desktop() {
    let id = ClientId::desktop();
    assert_eq!(id.as_str(), "sp_desktop");
}

#[test]
fn test_client_id_system() {
    let id = ClientId::system("scheduler");
    assert_eq!(id.as_str(), "sys_scheduler");
}

#[test]
fn test_client_id_display() {
    let id = ClientId::new("display-client");
    assert_eq!(format!("{}", id), "display-client");
}

#[test]
fn test_client_id_from_string() {
    let id: ClientId = String::from("from-string-client").into();
    assert_eq!(id.as_str(), "from-string-client");
}

#[test]
fn test_client_id_from_str() {
    let id: ClientId = "from-str-client".into();
    assert_eq!(id.as_str(), "from-str-client");
}

#[test]
fn test_client_id_as_ref() {
    let id = ClientId::new("as-ref-client");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-client");
}

#[test]
fn test_client_id_clone_and_eq() {
    let id1 = ClientId::new("clone-client");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_client_id_hash() {
    let id1 = ClientId::new("hash-client");
    let id2 = ClientId::new("hash-client");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_client_id_serialize_json() {
    let id = ClientId::new("serialize-client");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-client\"");
}

#[test]
fn test_client_id_deserialize_json() {
    let id: ClientId = serde_json::from_str("\"deserialize-client\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-client");
}

#[test]
fn test_client_id_to_db_value() {
    let id = ClientId::new("db-value-client");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-client"));
}

#[test]
fn test_client_id_ref_to_db_value() {
    let id = ClientId::new("db-value-ref-client");
    let db_value = (&id).to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-ref-client"));
}

// ============================================================================
// ClientId client_type() Tests
// ============================================================================

#[test]
fn test_client_type_cimd() {
    let id = ClientId::new("https://example.com/mcp");
    assert_eq!(id.client_type(), ClientType::Cimd);
}

#[test]
fn test_client_type_first_party() {
    let id = ClientId::new("sp_web");
    assert_eq!(id.client_type(), ClientType::FirstParty);
}

#[test]
fn test_client_type_third_party() {
    let id = ClientId::new("client_abc123");
    assert_eq!(id.client_type(), ClientType::ThirdParty);
}

#[test]
fn test_client_type_system() {
    let id = ClientId::new("sys_scheduler");
    assert_eq!(id.client_type(), ClientType::System);
}

#[test]
fn test_client_type_unknown() {
    let id = ClientId::new("random-client-id");
    assert_eq!(id.client_type(), ClientType::Unknown);
}

#[test]
fn test_client_id_is_dcr_first_party() {
    let id = ClientId::web();
    assert!(id.is_dcr());
}

#[test]
fn test_client_id_is_dcr_third_party() {
    let id = ClientId::new("client_third");
    assert!(id.is_dcr());
}

#[test]
fn test_client_id_is_dcr_false_for_cimd() {
    let id = ClientId::new("https://example.com");
    assert!(!id.is_dcr());
}

#[test]
fn test_client_id_is_dcr_false_for_system() {
    let id = ClientId::system("test");
    assert!(!id.is_dcr());
}

#[test]
fn test_client_id_is_cimd() {
    let id = ClientId::new("https://example.com/endpoint");
    assert!(id.is_cimd());
}

#[test]
fn test_client_id_is_cimd_false() {
    let id = ClientId::web();
    assert!(!id.is_cimd());
}

#[test]
fn test_client_id_is_system() {
    let id = ClientId::system("worker");
    assert!(id.is_system());
}

#[test]
fn test_client_id_is_system_false() {
    let id = ClientId::web();
    assert!(!id.is_system());
}

// ============================================================================
// ClientType Tests
// ============================================================================

#[test]
fn test_client_type_as_str_cimd() {
    assert_eq!(ClientType::Cimd.as_str(), "cimd");
}

#[test]
fn test_client_type_as_str_first_party() {
    assert_eq!(ClientType::FirstParty.as_str(), "firstparty");
}

#[test]
fn test_client_type_as_str_third_party() {
    assert_eq!(ClientType::ThirdParty.as_str(), "thirdparty");
}

#[test]
fn test_client_type_as_str_system() {
    assert_eq!(ClientType::System.as_str(), "system");
}

#[test]
fn test_client_type_as_str_unknown() {
    assert_eq!(ClientType::Unknown.as_str(), "unknown");
}

#[test]
fn test_client_type_display() {
    assert_eq!(format!("{}", ClientType::Cimd), "cimd");
    assert_eq!(format!("{}", ClientType::FirstParty), "firstparty");
    assert_eq!(format!("{}", ClientType::ThirdParty), "thirdparty");
    assert_eq!(format!("{}", ClientType::System), "system");
    assert_eq!(format!("{}", ClientType::Unknown), "unknown");
}

#[test]
fn test_client_type_clone() {
    let ct1 = ClientType::FirstParty;
    let ct2 = ct1.clone();
    assert_eq!(ct1, ct2);
}

#[test]
fn test_client_type_copy() {
    let ct1 = ClientType::System;
    let ct2 = ct1; // Copy
    assert_eq!(ct1, ct2);
}

#[test]
fn test_client_type_debug() {
    let debug_str = format!("{:?}", ClientType::Cimd);
    assert!(debug_str.contains("Cimd"));
}

#[test]
fn test_client_type_serialize_json() {
    let json = serde_json::to_string(&ClientType::FirstParty).unwrap();
    assert_eq!(json, "\"firstparty\"");
}

#[test]
fn test_client_type_deserialize_json() {
    let ct: ClientType = serde_json::from_str("\"thirdparty\"").unwrap();
    assert_eq!(ct, ClientType::ThirdParty);
}

#[test]
fn test_client_type_deserialize_all_variants() {
    let cimd: ClientType = serde_json::from_str("\"cimd\"").unwrap();
    let first: ClientType = serde_json::from_str("\"firstparty\"").unwrap();
    let third: ClientType = serde_json::from_str("\"thirdparty\"").unwrap();
    let system: ClientType = serde_json::from_str("\"system\"").unwrap();
    let unknown: ClientType = serde_json::from_str("\"unknown\"").unwrap();

    assert_eq!(cimd, ClientType::Cimd);
    assert_eq!(first, ClientType::FirstParty);
    assert_eq!(third, ClientType::ThirdParty);
    assert_eq!(system, ClientType::System);
    assert_eq!(unknown, ClientType::Unknown);
}
