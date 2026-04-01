//! Unit tests for repository types and traits

use systemprompt_database::repository::EntityId;

// ============================================================================
// EntityId Trait Tests
// ============================================================================

#[test]
fn test_string_implements_entity_id() {
    let id = String::from_string("test-id".to_string());
    assert_eq!(id.as_str(), "test-id");
}
