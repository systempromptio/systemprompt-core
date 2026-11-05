//! [`SubjectLinkRepository`]: `brain_subject_links`, the bridges a fact
//! declares between its own subject and every other subject one of its
//! entity fields names. A row is written in both directions
//! (`subject -> other` and `other -> subject`) with the fact's kind as
//! `via_kind`, so the linked tier at depth 2 reaches the facts about and
//! linked to a bridged subject through one lookup by subject. The rows
//! are derived in SQL from `brain_fact_links` (refreshed first for the
//! same set), so a set refresh and a full rebuild agree, and they cascade
//! with the fact.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{PgConnection, PgExecutor};
use systemprompt_models::brain::SubjectKey;
use systemprompt_traits::RepositoryError;

/// One subject bridged from another, and the fact and kind that declare it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubjectLink {
    pub other: SubjectKey,
    pub via_kind: String,
    pub fact_id: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SubjectLinkRepository;

impl SubjectLinkRepository {
    pub async fn refresh_many(
        conn: &mut PgConnection,
        ids: &[String],
    ) -> Result<u64, RepositoryError> {
        sqlx::query!(
            "DELETE FROM brain_subject_links WHERE fact_id = ANY($1::text[])",
            ids
        )
        .execute(&mut *conn)
        .await?;
        let result = sqlx::query!(
            r"INSERT INTO brain_subject_links
                  (subject_kind, subject_id, other_kind, other_id, via_kind, fact_id)
              SELECT f.subject_kind, f.subject_id, l.subject_kind, l.subject_id, f.kind, f.id
              FROM brain_facts f
              JOIN brain_fact_links l ON l.fact_id = f.id
              WHERE f.id = ANY($1::text[])
                AND l.via_field <> 'subject'
                AND NOT (l.subject_kind = f.subject_kind AND l.subject_id = f.subject_id)
              UNION
              SELECT l.subject_kind, l.subject_id, f.subject_kind, f.subject_id, f.kind, f.id
              FROM brain_facts f
              JOIN brain_fact_links l ON l.fact_id = f.id
              WHERE f.id = ANY($1::text[])
                AND l.via_field <> 'subject'
                AND NOT (l.subject_kind = f.subject_kind AND l.subject_id = f.subject_id)
              ON CONFLICT DO NOTHING",
            ids
        )
        .execute(conn)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn rebuild_all(conn: &mut PgConnection) -> Result<u64, RepositoryError> {
        sqlx::query!("DELETE FROM brain_subject_links")
            .execute(&mut *conn)
            .await?;
        let result = sqlx::query!(
            r"INSERT INTO brain_subject_links
                  (subject_kind, subject_id, other_kind, other_id, via_kind, fact_id)
              SELECT f.subject_kind, f.subject_id, l.subject_kind, l.subject_id, f.kind, f.id
              FROM brain_facts f
              JOIN brain_fact_links l ON l.fact_id = f.id
              WHERE l.via_field <> 'subject'
                AND NOT (l.subject_kind = f.subject_kind AND l.subject_id = f.subject_id)
              UNION
              SELECT l.subject_kind, l.subject_id, f.subject_kind, f.subject_id, f.kind, f.id
              FROM brain_facts f
              JOIN brain_fact_links l ON l.fact_id = f.id
              WHERE l.via_field <> 'subject'
                AND NOT (l.subject_kind = f.subject_kind AND l.subject_id = f.subject_id)
              ON CONFLICT DO NOTHING"
        )
        .execute(conn)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn list_for(
        executor: impl PgExecutor<'_>,
        subject: &SubjectKey,
    ) -> Result<Vec<SubjectLink>, RepositoryError> {
        let rows = sqlx::query!(
            "SELECT other_kind, other_id, via_kind, fact_id FROM brain_subject_links
             WHERE subject_kind = $1 AND subject_id = $2
             ORDER BY other_kind, other_id, fact_id",
            subject.kind.as_str(),
            subject.id.as_str()
        )
        .fetch_all(executor)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| SubjectLink {
                other: SubjectKey::new(row.other_kind, row.other_id),
                via_kind: row.via_kind,
                fact_id: row.fact_id,
            })
            .collect())
    }
}
