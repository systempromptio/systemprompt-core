//! [`WorkingSetRepository`]: the `brain_working_set` projection recall
//! reads first. One row per fact per scope key it is reachable through:
//! the fact's own scope (`org`, `user:<id>`, `project:<key>`,
//! `group:<key>`) and the audience subjects of its topics, in the
//! `AudienceSubject` display form with `everyone` folded into `org`.
//!
//! Status, review, validity and pinning stay on `brain_facts` and are
//! filtered at read, so the set changes only when a fact is inserted,
//! revised (scope, topics or importance), deleted (the foreign key
//! cascades) or when a topic's audience changes (the plane apply rebuilds).
//! The projection is derived entirely in SQL from `brain_facts` and
//! `brain_topics`, so a per-fact refresh and a full rebuild cannot disagree.
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
        sqlx::query!(
            "DELETE FROM brain_working_set WHERE fact_id = $1",
            id.as_str()
        )
        .execute(&mut *conn)
        .await?;
        let result = sqlx::query!(
            r"INSERT INTO brain_working_set (scope_key, fact_id, importance_base)
              SELECT DISTINCT s.key, f.id, f.importance_base
              FROM brain_facts f
              CROSS JOIN LATERAL (
                  SELECT CASE WHEN f.scope_level = 'org' THEN 'org'
                              ELSE f.scope_level || ':' || f.scope_key END AS key
                  UNION
                  SELECT CASE WHEN a = 'everyone' THEN 'org' ELSE a END
                  FROM brain_topics t,
                       jsonb_array_elements_text(COALESCE(t.body -> 'audience', '[]'::jsonb)) a
                  WHERE t.id = ANY (f.topics)
              ) s
              WHERE f.id = $1
              ON CONFLICT DO NOTHING",
            id.as_str()
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
              CROSS JOIN LATERAL (
                  SELECT CASE WHEN f.scope_level = 'org' THEN 'org'
                              ELSE f.scope_level || ':' || f.scope_key END AS key
                  UNION
                  SELECT CASE WHEN a = 'everyone' THEN 'org' ELSE a END
                  FROM brain_topics t,
                       jsonb_array_elements_text(COALESCE(t.body -> 'audience', '[]'::jsonb)) a
                  WHERE t.id = ANY (f.topics)
              ) s
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
                          CROSS JOIN LATERAL (
                              SELECT CASE WHEN f.scope_level = 'org' THEN 'org'
                                          ELSE f.scope_level || ':' || f.scope_key END AS key
                              UNION
                              SELECT CASE WHEN a = 'everyone' THEN 'org' ELSE a END
                              FROM brain_topics t,
                                   jsonb_array_elements_text(
                                       COALESCE(t.body -> 'audience', '[]'::jsonb)) a
                              WHERE t.id = ANY (f.topics)
                          ) s) expected) AS "expected!""#
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
