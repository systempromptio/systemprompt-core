//! [`WorkingSetRepository`]: the `brain_working_set` projection recall
//! reads first. One row per fact per scope key it is reachable through:
//! the fact's own scope (`org`, `user:<id>`, `project:<key>`,
//! `group:<key>`) and, for every scope but a user's, the audience subjects
//! of its topics, in the `AudienceSubject` display form with `everyone`
//! folded into `org`. A user-scoped fact is reachable only by its owner.
//! The keys come from the `brain_fact_scope_keys` SQL function, the SQL
//! half of `recall::visible::topic_widens`.
//!
//! Status, review, validity and pinning stay on `brain_facts` and are
//! filtered at read, so the set changes only when a fact is inserted,
//! revised (scope, topics or importance), deleted (the foreign key
//! cascades) or when a topic's audience changes (the plane apply rebuilds).
//! The projection is derived entirely in SQL from `brain_facts` and
//! `brain_topics`, so a per-fact refresh, a set refresh and a full rebuild
//! cannot disagree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{PgConnection, PgExecutor};
use systemprompt_identifiers::BrainFactId;
use systemprompt_traits::RepositoryError;

/// How many rows the projection holds against how many it should.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WorkingSetCounts {
    pub stored: i64,
    pub expected: i64,
}

impl WorkingSetCounts {
    #[must_use]
    pub const fn is_consistent(self) -> bool {
        self.stored == self.expected
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct WorkingSetRepository;

impl WorkingSetRepository {
    pub async fn refresh(
        conn: &mut PgConnection,
        id: &BrainFactId,
    ) -> Result<u64, RepositoryError> {
        Self::refresh_many(conn, &[id.as_str().to_owned()]).await
    }

    pub async fn refresh_many(
        conn: &mut PgConnection,
        ids: &[String],
    ) -> Result<u64, RepositoryError> {
        sqlx::query!(
            "DELETE FROM brain_working_set WHERE fact_id = ANY($1::text[])",
            ids
        )
        .execute(&mut *conn)
        .await?;
        let result = sqlx::query!(
            r"INSERT INTO brain_working_set (scope_key, fact_id, importance_base)
              SELECT DISTINCT s.key, f.id, f.importance_base
              FROM brain_facts f
              CROSS JOIN LATERAL brain_fact_scope_keys(f.scope_level, f.scope_key, f.topics) s(key)
              WHERE f.id = ANY($1::text[])
              ON CONFLICT DO NOTHING",
            ids
        )
        .execute(conn)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn rebuild_all(conn: &mut PgConnection) -> Result<u64, RepositoryError> {
        sqlx::query!("DELETE FROM brain_working_set")
            .execute(&mut *conn)
            .await?;
        let result = sqlx::query!(
            r"INSERT INTO brain_working_set (scope_key, fact_id, importance_base)
              SELECT DISTINCT s.key, f.id, f.importance_base
              FROM brain_facts f
              CROSS JOIN LATERAL brain_fact_scope_keys(f.scope_level, f.scope_key, f.topics) s(key)
              ON CONFLICT DO NOTHING"
        )
        .execute(conn)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn counts(
        executor: impl PgExecutor<'_>,
    ) -> Result<WorkingSetCounts, RepositoryError> {
        let row = sqlx::query!(
            r#"SELECT (SELECT count(*) FROM brain_working_set) AS "stored!",
                      (SELECT count(*) FROM (
                          SELECT DISTINCT s.key, f.id
                          FROM brain_facts f
                          CROSS JOIN LATERAL brain_fact_scope_keys(f.scope_level, f.scope_key, f.topics) s(key)) expected) AS "expected!""#
        )
        .fetch_one(executor)
        .await?;
        Ok(WorkingSetCounts {
            stored: row.stored,
            expected: row.expected,
        })
    }

    pub async fn keys_for(
        executor: impl PgExecutor<'_>,
        id: &BrainFactId,
    ) -> Result<Vec<String>, RepositoryError> {
        Ok(sqlx::query_scalar!(
            "SELECT scope_key FROM brain_working_set WHERE fact_id = $1 ORDER BY scope_key",
            id.as_str()
        )
        .fetch_all(executor)
        .await?)
    }

    pub async fn count_for(
        executor: impl PgExecutor<'_>,
        id: &BrainFactId,
    ) -> Result<i64, RepositoryError> {
        Ok(sqlx::query_scalar!(
            r#"SELECT count(*) AS "count!" FROM brain_working_set WHERE fact_id = $1"#,
            id.as_str()
        )
        .fetch_one(executor)
        .await?)
    }
}
