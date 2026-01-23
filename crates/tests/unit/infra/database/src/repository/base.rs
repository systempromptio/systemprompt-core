//! Unit tests for Repository and PaginatedRepository traits

use systemprompt_database::repository::{PgDbPool, Repository, PaginatedRepository};
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
// Repository Trait Tests
// ============================================================================

#[test]
fn test_repository_trait_is_object_safe() {
    trait ObjectSafe: Repository {}
}

#[test]
fn test_repository_associated_types() {
    trait CheckTypes: Repository {
        fn _check(&self)
        where
            Self::Entity: Send + Sync,
            Self::Id: Send + Sync,
            Self::Error: Send + Sync + std::error::Error,
        {
        }
    }
}

#[test]
fn test_repository_requires_send_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    fn _check_repository<R: Repository>() {
        assert_send_sync::<R>();
    }
}

// ============================================================================
// PaginatedRepository Trait Tests
// ============================================================================

#[test]
fn test_paginated_repository_extends_repository() {
    trait ExtendCheck: PaginatedRepository {}
    fn _check_extends_repository<R: PaginatedRepository>()
    where
        R: Repository,
    {
    }
}
