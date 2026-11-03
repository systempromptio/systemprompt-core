//! The brain config plane's tables: kinds, topics, interests, workflow
//! profiles; the config documents are in `plane_docs`. Each row
//! holds one declaration as JSONB under its id; the config plane writes
//! them inside the host's apply transaction and the pipeline reads them
//! once per drain round.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::{PgConnection, PgExecutor};
use systemprompt_identifiers::UserId;
use systemprompt_traits::RepositoryError;

use super::Pools;
use crate::models::{PlaneRow, PlaneSource};

/// One declaration to write.
#[derive(Debug, Clone, Copy)]
pub struct PlaneWrite<'a> {
    pub id: &'a str,
    // JSON: a plane row stores its YAML declaration verbatim as JSONB.
    pub body: &'a serde_json::Value,
    pub source: PlaneSource,
}

#[derive(Debug, Clone)]
pub struct KindRepository {
    pools: Pools,
}

impl KindRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn list(executor: impl PgExecutor<'_>) -> Result<Vec<PlaneRow>, RepositoryError> {
        Ok(sqlx::query_as!(
            PlaneRow,
            "SELECT id, body, source, updated_at FROM brain_fact_kinds ORDER BY id"
        )
        .fetch_all(executor)
        .await?)
    }

    pub async fn list_all(&self) -> Result<Vec<PlaneRow>, RepositoryError> {
        Self::list(&*self.pools.read).await
    }

    pub async fn upsert(
        conn: &mut PgConnection,
        row: &PlaneWrite<'_>,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            "INSERT INTO brain_fact_kinds (id, body, source) VALUES ($1, $2, $3)
             ON CONFLICT (id) DO UPDATE
             SET body = EXCLUDED.body, source = EXCLUDED.source, updated_at = now()",
            row.id,
            row.body,
            row.source.as_str(),
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn delete(conn: &mut PgConnection, id: &str) -> Result<bool, RepositoryError> {
        let result = sqlx::query!("DELETE FROM brain_fact_kinds WHERE id = $1", id)
            .execute(conn)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[derive(Debug, Clone)]
pub struct TopicRepository {
    pools: Pools,
}

impl TopicRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn list(executor: impl PgExecutor<'_>) -> Result<Vec<PlaneRow>, RepositoryError> {
        Ok(sqlx::query_as!(
            PlaneRow,
            "SELECT id, body, source, updated_at FROM brain_topics ORDER BY id"
        )
        .fetch_all(executor)
        .await?)
    }

    pub async fn list_all(&self) -> Result<Vec<PlaneRow>, RepositoryError> {
        Self::list(&*self.pools.read).await
    }

    pub async fn upsert(
        conn: &mut PgConnection,
        row: &PlaneWrite<'_>,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            "INSERT INTO brain_topics (id, body, source) VALUES ($1, $2, $3)
             ON CONFLICT (id) DO UPDATE
             SET body = EXCLUDED.body, source = EXCLUDED.source, updated_at = now()",
            row.id,
            row.body,
            row.source.as_str(),
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn delete(conn: &mut PgConnection, id: &str) -> Result<bool, RepositoryError> {
        let result = sqlx::query!("DELETE FROM brain_topics WHERE id = $1", id)
            .execute(conn)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[derive(Debug, Clone)]
pub struct InterestRepository {
    pools: Pools,
}

impl InterestRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn list(executor: impl PgExecutor<'_>) -> Result<Vec<PlaneRow>, RepositoryError> {
        Ok(sqlx::query_as!(
            PlaneRow,
            "SELECT id, body, source, updated_at FROM brain_interests ORDER BY id"
        )
        .fetch_all(executor)
        .await?)
    }

    pub async fn list_all(&self) -> Result<Vec<PlaneRow>, RepositoryError> {
        Self::list(&*self.pools.read).await
    }

    pub async fn upsert(
        conn: &mut PgConnection,
        row: &PlaneWrite<'_>,
        owner: Option<&UserId>,
        expires_at: Option<DateTime<Utc>>,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            "INSERT INTO brain_interests (id, body, source, owner_user_id, expires_at)
             VALUES ($1, $2, $3, $4, $5)
             ON CONFLICT (id) DO UPDATE
             SET body = EXCLUDED.body, source = EXCLUDED.source,
                 owner_user_id = EXCLUDED.owner_user_id, expires_at = EXCLUDED.expires_at,
                 updated_at = now()",
            row.id,
            row.body,
            row.source.as_str(),
            owner.map(UserId::as_str),
            expires_at,
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    /// The user a declared interest owner names: a `users.id`, or, for a
    /// value holding `@`, the user with that email (case-insensitive). An
    /// id match wins over an email match.
    pub async fn resolve_owner(
        conn: &mut PgConnection,
        owner: &UserId,
    ) -> Result<Option<UserId>, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT id AS "id: UserId" FROM users
               WHERE id = $1 OR (strpos($1, '@') > 0 AND email = lower(trim($1)))
               ORDER BY (id = $1) DESC
               LIMIT 1"#,
            owner.as_str(),
        )
        .fetch_optional(conn)
        .await?)
    }

    /// Each interest's resolved owner, as the plane apply stored it.
    pub async fn owners(
        executor: impl PgExecutor<'_>,
    ) -> Result<Vec<(String, UserId)>, RepositoryError> {
        let rows = sqlx::query!(
            r#"SELECT id, owner_user_id AS "owner!: UserId" FROM brain_interests
               WHERE owner_user_id IS NOT NULL"#
        )
        .fetch_all(executor)
        .await?;
        Ok(rows.into_iter().map(|row| (row.id, row.owner)).collect())
    }

    pub async fn delete(conn: &mut PgConnection, id: &str) -> Result<bool, RepositoryError> {
        let result = sqlx::query!("DELETE FROM brain_interests WHERE id = $1", id)
            .execute(conn)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[derive(Debug, Clone)]
pub struct WorkflowRepository {
    pools: Pools,
}

impl WorkflowRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn list(executor: impl PgExecutor<'_>) -> Result<Vec<PlaneRow>, RepositoryError> {
        Ok(sqlx::query_as!(
            PlaneRow,
            "SELECT id, body, source, updated_at FROM brain_workflows ORDER BY id"
        )
        .fetch_all(executor)
        .await?)
    }

    pub async fn list_all(&self) -> Result<Vec<PlaneRow>, RepositoryError> {
        Self::list(&*self.pools.read).await
    }

    pub async fn upsert(
        conn: &mut PgConnection,
        row: &PlaneWrite<'_>,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            "INSERT INTO brain_workflows (id, body, source) VALUES ($1, $2, $3)
             ON CONFLICT (id) DO UPDATE
             SET body = EXCLUDED.body, source = EXCLUDED.source, updated_at = now()",
            row.id,
            row.body,
            row.source.as_str(),
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn delete(conn: &mut PgConnection, id: &str) -> Result<bool, RepositoryError> {
        let result = sqlx::query!("DELETE FROM brain_workflows WHERE id = $1", id)
            .execute(conn)
            .await?;
        Ok(result.rows_affected() > 0)
    }
}
