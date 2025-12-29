//! Unit tests for AI-related identifier types.

use std::collections::HashSet;
use systemprompt_identifiers::{AiRequestId, MessageId, ConfigId, ToDbValue, DbValue};

// ============================================================================
// AiRequestId Tests
// ============================================================================

#[test]
fn test_ai_request_id_new() {
    let id = AiRequestId::new("req-123");
    assert_eq!(id.as_str(), "req-123");
}

#[test]
fn test_ai_request_id_generate() {
    let id = AiRequestId::generate();
    assert!(!id.as_str().is_empty());
    // UUID v4 format: 8-4-4-4-12
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn test_ai_request_id_generate_unique() {
    let id1 = AiRequestId::generate();
    let id2 = AiRequestId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn test_ai_request_id_display() {
    let id = AiRequestId::new("display-req");
    assert_eq!(format!("{}", id), "display-req");
}

#[test]
fn test_ai_request_id_from_string() {
    let id: AiRequestId = String::from("from-string-req").into();
    assert_eq!(id.as_str(), "from-string-req");
}

#[test]
fn test_ai_request_id_from_str() {
    let id: AiRequestId = "from-str-req".into();
    assert_eq!(id.as_str(), "from-str-req");
}

#[test]
fn test_ai_request_id_as_ref() {
    let id = AiRequestId::new("as-ref-req");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-req");
}

#[test]
fn test_ai_request_id_clone_and_eq() {
    let id1 = AiRequestId::new("clone-req");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_ai_request_id_hash() {
    let id1 = AiRequestId::new("hash-req");
    let id2 = AiRequestId::new("hash-req");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_ai_request_id_serialize_json() {
    let id = AiRequestId::new("serialize-req");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-req\"");
}

#[test]
fn test_ai_request_id_deserialize_json() {
    let id: AiRequestId = serde_json::from_str("\"deserialize-req\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-req");
}

#[test]
fn test_ai_request_id_to_db_value() {
    let id = AiRequestId::new("db-value-req");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-req"));
}

#[test]
fn test_ai_request_id_ref_to_db_value() {
    let id = AiRequestId::new("db-value-ref-req");
    let db_value = (&id).to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-ref-req"));
}

// ============================================================================
// MessageId Tests
// ============================================================================

#[test]
fn test_message_id_new() {
    let id = MessageId::new("msg-123");
    assert_eq!(id.as_str(), "msg-123");
}

#[test]
fn test_message_id_generate() {
    let id = MessageId::generate();
    assert!(!id.as_str().is_empty());
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn test_message_id_generate_unique() {
    let id1 = MessageId::generate();
    let id2 = MessageId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn test_message_id_display() {
    let id = MessageId::new("display-msg");
    assert_eq!(format!("{}", id), "display-msg");
}

#[test]
fn test_message_id_from_string() {
    let id: MessageId = String::from("from-string-msg").into();
    assert_eq!(id.as_str(), "from-string-msg");
}

#[test]
fn test_message_id_from_str() {
    let id: MessageId = "from-str-msg".into();
    assert_eq!(id.as_str(), "from-str-msg");
}

#[test]
fn test_message_id_as_ref() {
    let id = MessageId::new("as-ref-msg");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-msg");
}

#[test]
fn test_message_id_clone_and_eq() {
    let id1 = MessageId::new("clone-msg");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_message_id_hash() {
    let id1 = MessageId::new("hash-msg");
    let id2 = MessageId::new("hash-msg");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_message_id_serialize_json() {
    let id = MessageId::new("serialize-msg");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-msg\"");
}

#[test]
fn test_message_id_deserialize_json() {
    let id: MessageId = serde_json::from_str("\"deserialize-msg\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-msg");
}

#[test]
fn test_message_id_to_db_value() {
    let id = MessageId::new("db-value-msg");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-msg"));
}

// ============================================================================
// ConfigId Tests
// ============================================================================

#[test]
fn test_config_id_new() {
    let id = ConfigId::new("cfg-123");
    assert_eq!(id.as_str(), "cfg-123");
}

#[test]
fn test_config_id_generate() {
    let id = ConfigId::generate();
    assert!(!id.as_str().is_empty());
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn test_config_id_generate_unique() {
    let id1 = ConfigId::generate();
    let id2 = ConfigId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn test_config_id_display() {
    let id = ConfigId::new("display-cfg");
    assert_eq!(format!("{}", id), "display-cfg");
}

#[test]
fn test_config_id_from_string() {
    let id: ConfigId = String::from("from-string-cfg").into();
    assert_eq!(id.as_str(), "from-string-cfg");
}

#[test]
fn test_config_id_from_str() {
    let id: ConfigId = "from-str-cfg".into();
    assert_eq!(id.as_str(), "from-str-cfg");
}

#[test]
fn test_config_id_as_ref() {
    let id = ConfigId::new("as-ref-cfg");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-cfg");
}

#[test]
fn test_config_id_clone_and_eq() {
    let id1 = ConfigId::new("clone-cfg");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_config_id_hash() {
    let id1 = ConfigId::new("hash-cfg");
    let id2 = ConfigId::new("hash-cfg");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_config_id_serialize_json() {
    let id = ConfigId::new("serialize-cfg");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-cfg\"");
}

#[test]
fn test_config_id_deserialize_json() {
    let id: ConfigId = serde_json::from_str("\"deserialize-cfg\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-cfg");
}

#[test]
fn test_config_id_to_db_value() {
    let id = ConfigId::new("db-value-cfg");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-cfg"));
}
