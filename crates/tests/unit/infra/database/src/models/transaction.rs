//! Unit tests for DatabaseTransaction trait and BoxFuture type

use systemprompt_database::BoxFuture;

// ============================================================================
// BoxFuture Type Tests
// ============================================================================

#[test]
fn test_box_future_type_compiles() {
    fn _assert_future_type<T>(_: BoxFuture<'_, T>) {}
}

#[test]
fn test_box_future_is_send() {
    fn assert_send<T: Send>() {}
    assert_send::<BoxFuture<'static, ()>>();
}

// ============================================================================
// DatabaseTransaction Trait Tests
// ============================================================================

#[test]
fn test_database_transaction_trait_is_object_safe() {
    use systemprompt_database::DatabaseTransaction;
    fn _assert_object_safe(_: &dyn DatabaseTransaction) {}
}
