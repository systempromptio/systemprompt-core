use async_trait::async_trait;
use sqlx::postgres::PgRow;
use sqlx::{FromRow, PgPool};
use std::sync::Arc;

pub trait EntityId: Send + Sync + Clone + 'static {
    fn as_str(&self) -> &str;

    fn from_string(s: String) -> Self;
}

impl EntityId for String {
    fn as_str(&self) -> &str {
        self
    }

    fn from_string(s: String) -> Self {
        s
    }
}

pub trait Entity: for<'r> FromRow<'r, PgRow> + Send + Sync + Unpin + 'static {
    type Id: EntityId;

    const TABLE: &'static str;

    const COLUMNS: &'static str;

    const ID_COLUMN: &'static str;

    fn id(&self) -> &Self::Id;
}

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

    #[must_use]
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

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

    pub async fn list_all(&self) -> Result<Vec<E>, sqlx::Error> {
        let query = format!(
            "SELECT {} FROM {} ORDER BY created_at DESC",
            E::COLUMNS,
            E::TABLE
        );
        sqlx::query_as::<_, E>(&query).fetch_all(&*self.pool).await
    }

    pub async fn delete(&self, id: &E::Id) -> Result<bool, sqlx::Error> {
        let query = format!("DELETE FROM {} WHERE {} = $1", E::TABLE, E::ID_COLUMN);
        let result = sqlx::query(&query)
            .bind(id.as_str())
            .execute(&*self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn exists(&self, id: &E::Id) -> Result<bool, sqlx::Error> {
        let query = format!("SELECT 1 FROM {} WHERE {} = $1", E::TABLE, E::ID_COLUMN);
        let result: Option<(i32,)> = sqlx::query_as(&query)
            .bind(id.as_str())
            .fetch_optional(&*self.pool)
            .await?;
        Ok(result.is_some())
    }

    pub async fn count(&self) -> Result<i64, sqlx::Error> {
        let query = format!("SELECT COUNT(*) FROM {}", E::TABLE);
        let result: (i64,) = sqlx::query_as(&query).fetch_one(&*self.pool).await?;
        Ok(result.0)
    }
}

#[async_trait]
pub trait RepositoryExt<E: Entity>: Sized {
    fn pool(&self) -> &PgPool;

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
