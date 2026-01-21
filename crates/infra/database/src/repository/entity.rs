//! Generic entity-based repository for CRUD operations.
//!
//! This module provides a generic repository pattern that works with any
//! entity type that implements the `Entity` trait.

use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::{FromRow, PgPool};
use std::sync::Arc;

/// Trait for entity ID types.
///
/// Entity IDs must be convertible to/from strings for database operations.
pub trait EntityId: Send + Sync + Clone + 'static {
    /// Convert the ID to a string representation.
    fn as_str(&self) -> &str;

    /// Create an ID from a string.
    fn from_string(s: String) -> Self;
}

/// Implement `EntityId` for String.
impl EntityId for String {
    fn as_str(&self) -> &str {
        self
    }

    fn from_string(s: String) -> Self {
        s
    }
}

/// Trait for entities that can be stored in a repository.
///
/// Entities must implement `FromRow` for database deserialization and
/// provide metadata about their table structure.
///
/// # Example
///
/// ```rust,ignore
/// use systemprompt_database::repository::{Entity, EntityId};
/// use sqlx::FromRow;
///
/// #[derive(Debug, Clone, FromRow)]
/// pub struct User {
///     pub id: String,
///     pub email: String,
///     pub name: String,
/// }
///
/// impl Entity for User {
///     type Id = String;
///
///     const TABLE: &'static str = "users";
///     const COLUMNS: &'static str = "id, email, name";
///     const ID_COLUMN: &'static str = "id";
///
///     fn id(&self) -> &Self::Id {
///         &self.id
///     }
/// }
/// ```
pub trait Entity: for<'r> FromRow<'r, PgRow> + Send + Sync + Unpin + 'static {
    /// The ID type for this entity.
    type Id: EntityId;

    /// Database table name.
    const TABLE: &'static str;

    /// SQL column list for SELECT queries.
    const COLUMNS: &'static str;

    /// Name of the ID column.
    const ID_COLUMN: &'static str;

    /// Get the entity's ID.
    fn id(&self) -> &Self::Id;
}

/// Generic repository providing common CRUD operations.
///
/// This repository uses the `Entity` trait to generate SQL queries
/// dynamically based on the entity's table metadata.
#[derive(Clone)]
pub struct GenericRepository<E: Entity> {
    pool: Arc<PgPool>,
    _phantom: std::marker::PhantomData<E>,
}

impl<E: Entity> std::fmt::Debug for GenericRepository<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GenericRepository")
            .field("table", &E::TABLE)
            .finish()
    }
}

impl<E: Entity> GenericRepository<E> {
    #[must_use]
    pub const fn new(pool: Arc<PgPool>) -> Self {
        Self {
            pool,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Get the database pool.
    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Get an entity by its ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn get(&self, id: &E::Id) -> Result<Option<E>, sqlx::Error> {
        let query = format!(
            "SELECT {} FROM {} WHERE {} = $1",
            E::COLUMNS,
            E::TABLE,
            E::ID_COLUMN
        );
        sqlx::query_as::<_, E>(&query)
            .bind(id.as_str())
            .fetch_optional(&*self.pool)
            .await
    }

    /// List entities with pagination.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn list(&self, limit: i64, offset: i64) -> Result<Vec<E>, sqlx::Error> {
        let query = format!(
            "SELECT {} FROM {} ORDER BY created_at DESC LIMIT $1 OFFSET $2",
            E::COLUMNS,
            E::TABLE
        );
        sqlx::query_as::<_, E>(&query)
            .bind(limit)
            .bind(offset)
            .fetch_all(&*self.pool)
            .await
    }

    /// List all entities without pagination.
    ///
    /// Use with caution for large tables.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn list_all(&self) -> Result<Vec<E>, sqlx::Error> {
        let query = format!(
            "SELECT {} FROM {} ORDER BY created_at DESC",
            E::COLUMNS,
            E::TABLE
        );
        sqlx::query_as::<_, E>(&query).fetch_all(&*self.pool).await
    }

    /// Delete an entity by ID.
    ///
    /// Returns `true` if a row was deleted, `false` if no matching row was
    /// found.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn delete(&self, id: &E::Id) -> Result<bool, sqlx::Error> {
        let query = format!("DELETE FROM {} WHERE {} = $1", E::TABLE, E::ID_COLUMN);
        let result = sqlx::query(&query)
            .bind(id.as_str())
            .execute(&*self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Check if an entity exists by ID.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn exists(&self, id: &E::Id) -> Result<bool, sqlx::Error> {
        let query = format!("SELECT 1 FROM {} WHERE {} = $1", E::TABLE, E::ID_COLUMN);
        let result: Option<(i32,)> = sqlx::query_as(&query)
            .bind(id.as_str())
            .fetch_optional(&*self.pool)
            .await?;
        Ok(result.is_some())
    }

    /// Count total entities.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    pub async fn count(&self) -> Result<i64, sqlx::Error> {
        let query = format!("SELECT COUNT(*) FROM {}", E::TABLE);
        let result: (i64,) = sqlx::query_as(&query).fetch_one(&*self.pool).await?;
        Ok(result.0)
    }
}

/// Extension trait for custom queries on repositories.
#[async_trait]
pub trait RepositoryExt<E: Entity>: Sized {
    /// Get the database pool.
    fn pool(&self) -> &PgPool;

    /// Find an entity by a specific column value.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    async fn find_by<T: ToString + Send + Sync>(
        &self,
        column: &str,
        value: T,
    ) -> Result<Option<E>, sqlx::Error> {
        let query = format!(
            "SELECT {} FROM {} WHERE {} = $1",
            E::COLUMNS,
            E::TABLE,
            column
        );
        sqlx::query_as::<_, E>(&query)
            .bind(value.to_string())
            .fetch_optional(self.pool())
            .await
    }

    /// Find all entities matching a column value.
    ///
    /// # Errors
    ///
    /// Returns an error if the database query fails.
    async fn find_all_by<T: ToString + Send + Sync>(
        &self,
        column: &str,
        value: T,
    ) -> Result<Vec<E>, sqlx::Error> {
        let query = format!(
            "SELECT {} FROM {} WHERE {} = $1 ORDER BY created_at DESC",
            E::COLUMNS,
            E::TABLE,
            column
        );
        sqlx::query_as::<_, E>(&query)
            .bind(value.to_string())
            .fetch_all(self.pool())
            .await
    }
}

impl<E: Entity> RepositoryExt<E> for GenericRepository<E> {
    fn pool(&self) -> &PgPool {
        &self.pool
    }
}
