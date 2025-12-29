//! Unit tests for AgentId and AgentName types.

use std::collections::HashSet;
use systemprompt_identifiers::{AgentId, AgentName, ToDbValue, DbValue};

// ============================================================================
// AgentId Tests
// ============================================================================

#[test]
fn test_agent_id_new_from_string() {
    let id = AgentId::new("test-agent-123");
    assert_eq!(id.as_str(), "test-agent-123");
}

#[test]
fn test_agent_id_new_from_owned_string() {
    let id = AgentId::new(String::from("test-agent-456"));
    assert_eq!(id.as_str(), "test-agent-456");
}

#[test]
fn test_agent_id_display() {
    let id = AgentId::new("display-test");
    assert_eq!(format!("{}", id), "display-test");
}

#[test]
fn test_agent_id_from_string() {
    let id: AgentId = String::from("from-string").into();
    assert_eq!(id.as_str(), "from-string");
}

#[test]
fn test_agent_id_from_str() {
    let id: AgentId = "from-str".into();
    assert_eq!(id.as_str(), "from-str");
}

#[test]
fn test_agent_id_as_ref() {
    let id = AgentId::new("as-ref-test");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-test");
}

#[test]
fn test_agent_id_clone() {
    let id1 = AgentId::new("clone-test");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_agent_id_equality() {
    let id1 = AgentId::new("equal");
    let id2 = AgentId::new("equal");
    let id3 = AgentId::new("different");

    assert_eq!(id1, id2);
    assert_ne!(id1, id3);
}

#[test]
fn test_agent_id_hash() {
    let id1 = AgentId::new("hash-test");
    let id2 = AgentId::new("hash-test");

    let mut set = HashSet::new();
    set.insert(id1.clone());

    assert!(set.contains(&id2));
}

#[test]
fn test_agent_id_serialize_json() {
    let id = AgentId::new("serialize-test");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-test\"");
}

#[test]
fn test_agent_id_deserialize_json() {
    let id: AgentId = serde_json::from_str("\"deserialize-test\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-test");
}

#[test]
fn test_agent_id_to_db_value() {
    let id = AgentId::new("db-value-test");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-test"));
}

#[test]
fn test_agent_id_ref_to_db_value() {
    let id = AgentId::new("db-value-ref-test");
    let db_value = (&id).to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-ref-test"));
}

#[test]
fn test_agent_id_debug() {
    let id = AgentId::new("debug-test");
    let debug_str = format!("{:?}", id);
    assert!(debug_str.contains("AgentId"));
    assert!(debug_str.contains("debug-test"));
}

#[test]
fn test_agent_id_empty_string() {
    let id = AgentId::new("");
    assert_eq!(id.as_str(), "");
}

#[test]
fn test_agent_id_unicode() {
    let id = AgentId::new("agent-日本語-emoji-🤖");
    assert_eq!(id.as_str(), "agent-日本語-emoji-🤖");
}

// ============================================================================
// AgentName Tests
// ============================================================================

#[test]
fn test_agent_name_new_valid() {
    let name = AgentName::new("valid-agent");
    assert_eq!(name.as_str(), "valid-agent");
}

#[test]
fn test_agent_name_system() {
    let name = AgentName::system();
    assert_eq!(name.as_str(), "system");
}

#[test]
fn test_agent_name_display() {
    let name = AgentName::new("display-agent");
    assert_eq!(format!("{}", name), "display-agent");
}

#[test]
fn test_agent_name_from_string() {
    let name: AgentName = String::from("from-string-agent").into();
    assert_eq!(name.as_str(), "from-string-agent");
}

#[test]
fn test_agent_name_from_str() {
    let name: AgentName = "from-str-agent".into();
    assert_eq!(name.as_str(), "from-str-agent");
}

#[test]
fn test_agent_name_as_ref() {
    let name = AgentName::new("as-ref-agent");
    let s: &str = name.as_ref();
    assert_eq!(s, "as-ref-agent");
}

#[test]
fn test_agent_name_clone() {
    let name1 = AgentName::new("clone-agent");
    let name2 = name1.clone();
    assert_eq!(name1, name2);
}

#[test]
fn test_agent_name_equality() {
    let name1 = AgentName::new("equal-agent");
    let name2 = AgentName::new("equal-agent");
    let name3 = AgentName::new("different-agent");

    assert_eq!(name1, name2);
    assert_ne!(name1, name3);
}

#[test]
fn test_agent_name_hash() {
    let name1 = AgentName::new("hash-agent");
    let name2 = AgentName::new("hash-agent");

    let mut set = HashSet::new();
    set.insert(name1.clone());

    assert!(set.contains(&name2));
}

#[test]
fn test_agent_name_serialize_json() {
    let name = AgentName::new("serialize-agent");
    let json = serde_json::to_string(&name).unwrap();
    assert_eq!(json, "\"serialize-agent\"");
}

#[test]
fn test_agent_name_deserialize_json() {
    let name: AgentName = serde_json::from_str("\"deserialize-agent\"").unwrap();
    assert_eq!(name.as_str(), "deserialize-agent");
}

#[test]
fn test_agent_name_to_db_value() {
    let name = AgentName::new("db-value-agent");
    let db_value = name.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-agent"));
}

#[test]
fn test_agent_name_ref_to_db_value() {
    let name = AgentName::new("db-value-ref-agent");
    let db_value = (&name).to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-ref-agent"));
}

#[test]
fn test_agent_name_debug() {
    let name = AgentName::new("debug-agent");
    let debug_str = format!("{:?}", name);
    assert!(debug_str.contains("AgentName"));
    assert!(debug_str.contains("debug-agent"));
}

#[test]
#[should_panic(expected = "Agent name cannot be empty")]
fn test_agent_name_empty_panics() {
    let _ = AgentName::new("");
}

#[test]
#[should_panic(expected = "Agent name 'unknown' is reserved")]
fn test_agent_name_unknown_panics() {
    let _ = AgentName::new("unknown");
}

#[test]
#[should_panic(expected = "Agent name 'unknown' is reserved")]
fn test_agent_name_unknown_uppercase_panics() {
    let _ = AgentName::new("UNKNOWN");
}

#[test]
#[should_panic(expected = "Agent name 'unknown' is reserved")]
fn test_agent_name_unknown_mixed_case_panics() {
    let _ = AgentName::new("UnKnOwN");
}

#[test]
fn test_agent_name_hyphenated() {
    let name = AgentName::new("content-research");
    assert_eq!(name.as_str(), "content-research");
}

#[test]
fn test_agent_name_edward() {
    let name = AgentName::new("edward");
    assert_eq!(name.as_str(), "edward");
}
