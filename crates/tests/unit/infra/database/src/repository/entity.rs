//! Unit tests for Entity and EntityId traits

use systemprompt_core_database::repository::EntityId;

// ============================================================================
// EntityId for String Tests
// ============================================================================

#[test]
fn test_string_entity_id_as_str() {
    let id = String::from("user-123");
    assert_eq!(id.as_str(), "user-123");
}

#[test]
fn test_string_entity_id_from_string() {
    let id = String::from_string("entity-456".to_string());
    assert_eq!(id, "entity-456");
}

#[test]
fn test_string_entity_id_empty() {
    let id = String::from_string(String::new());
    assert_eq!(id.as_str(), "");
}

#[test]
fn test_string_entity_id_with_special_chars() {
    let id = String::from_string("id-with-dashes_and_underscores".to_string());
    assert_eq!(id.as_str(), "id-with-dashes_and_underscores");
}

#[test]
fn test_string_entity_id_uuid_like() {
    let id = String::from_string("550e8400-e29b-41d4-a716-446655440000".to_string());
    assert!(id.as_str().contains("-"));
    assert_eq!(id.len(), 36);
}

#[test]
fn test_string_entity_id_clone() {
    let id = String::from_string("original-id".to_string());
    let cloned = id.clone();
    assert_eq!(id.as_str(), cloned.as_str());
}

#[test]
fn test_string_entity_id_roundtrip() {
    let original = "test-entity-id";
    let id = String::from_string(original.to_string());
    assert_eq!(id.as_str(), original);
}

// ============================================================================
// EntityId Send + Sync Tests
// ============================================================================

#[test]
fn test_string_entity_id_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<String>();
}

#[test]
fn test_string_entity_id_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<String>();
}

// ============================================================================
// EntityId Edge Cases
// ============================================================================

#[test]
fn test_string_entity_id_with_unicode() {
    let id = String::from_string("用户-123".to_string());
    assert_eq!(id.as_str(), "用户-123");
}

#[test]
fn test_string_entity_id_with_spaces() {
    let id = String::from_string("id with spaces".to_string());
    assert_eq!(id.as_str(), "id with spaces");
}

#[test]
fn test_string_entity_id_numeric() {
    let id = String::from_string("12345".to_string());
    assert_eq!(id.as_str(), "12345");
}
