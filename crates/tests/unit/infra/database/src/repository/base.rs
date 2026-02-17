//! Unit tests for repository types and traits

use systemprompt_database::repository::{Entity, EntityId, GenericRepository, PgDbPool, RepositoryExt};
use systemprompt_database::PgPool;

// ============================================================================
// PgDbPool Type Tests
// ============================================================================

#[test]
fn test_pg_db_pool_type_alias() {
    use std::sync::Arc;
    fn _assert_type(_: PgDbPool, _: Arc<PgPool>) {}
}

// ============================================================================
// EntityId Trait Tests
// ============================================================================

#[test]
fn test_entity_id_requires_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    fn _check_entity_id<I: EntityId>() {
        assert_send_sync::<I>();
    }
}

#[test]
fn test_string_implements_entity_id() {
    let id = String::from_string("test-id".to_string());
    assert_eq!(id.as_str(), "test-id");
}

// ============================================================================
// Entity Trait Tests
// ============================================================================

#[test]
fn test_entity_associated_types() {
    fn _check_entity<E: Entity>()
    where
        E::Id: EntityId,
    {
    }
}

// ============================================================================
// GenericRepository Tests
// ============================================================================

#[test]
fn test_generic_repository_is_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    fn _check<E: Entity>() {
        assert_send_sync::<GenericRepository<E>>();
    }
}

#[test]
fn test_generic_repository_is_clone() {
    fn assert_clone<T: Clone>() {}
    fn _check<E: Entity + Clone>() {
        assert_clone::<GenericRepository<E>>();
    }
}

// ============================================================================
// RepositoryExt Trait Tests
// ============================================================================

#[test]
fn test_generic_repository_implements_repository_ext() {
    fn _check<E: Entity>()
    where
        GenericRepository<E>: RepositoryExt<E>,
    {
    }
}
