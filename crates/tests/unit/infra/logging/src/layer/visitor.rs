//! Unit tests for DatabaseLayer
//!
//! Note: The visitor types (FieldVisitor, SpanContext, SpanVisitor, SpanFields)
//! are internal to the layer module and not publicly exported.
//! These tests focus on the publicly available DatabaseLayer functionality.

use systemprompt_logging::DatabaseLayer;

// ============================================================================
// DatabaseLayer Existence Test
// ============================================================================

#[test]
fn test_database_layer_type_exists() {
    fn assert_is_type<T>() {}
    assert_is_type::<DatabaseLayer>();
}

// ============================================================================
// DatabaseLayer Trait Bounds Tests
// ============================================================================

#[test]
fn test_database_layer_is_debug() {
    fn assert_debug<T: std::fmt::Debug>() {}
    assert_debug::<DatabaseLayer>();
}

#[test]
fn test_database_layer_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<DatabaseLayer>();
}

#[test]
fn test_database_layer_is_sync() {
    fn assert_sync<T: Sync>() {}
    assert_sync::<DatabaseLayer>();
}

// ============================================================================
// Note on Further Testing
// ============================================================================

// Integration tests for DatabaseLayer that require:
// - Creating a DbPool
// - Testing the Layer trait implementation (on_event, on_new_span, on_record)
// - Testing batch writing and flushing
//
// Should be placed in integration tests with proper database setup.
// See: crates/tests/integration/database/ for database testing patterns
