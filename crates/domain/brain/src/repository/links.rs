//! [`FactLinkRepository`]: `brain_fact_links`, the subjects a fact
//! mentions. Every fact links its own subject through `via_field =
//! 'subject'`, and every `entity` field that carries a value links the
//! subject that value names: `<ref>:<id>` as written, or the field's
//! declared `ref` kind with the bare value. The rows are derived in SQL
//! from the fact and its kind declaration, so a refresh and the migration
//! backfill agree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::{PgConnection, PgExecutor};
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::SubjectKey;
use systemprompt_traits::RepositoryError;

/// One subject a fact links to and the field that names it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactLink {
    pub subject: SubjectKey,
    pub via_field: String,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct FactLinkRepository;

impl FactLinkRepository {
    pub async fn refresh(
        conn: &mut PgConnection,
        id: &BrainFactId,
    ) -> Result<u64, RepositoryError> {
        sqlx::query!(
            "DELETE FROM brain_fact_links WHERE fact_id = $1",
            id.as_str()
        )
        .execute(&mut *conn)
        .await?;
        let result = sqlx::query!(
            r"INSERT INTO brain_fact_links (fact_id, subject_kind, subject_id, via_field)
              SELECT f.id, f.subject_kind, f.subject_id, 'subject' FROM brain_facts f
              WHERE f.id = $1
              UNION
              SELECT f.id,
                     CASE WHEN strpos(x.v, ':') > 0 THEN split_part(x.v, ':', 1)
                          ELSE d.decl ->> 'ref' END,
                     CASE WHEN strpos(x.v, ':') > 0 THEN substr(x.v, strpos(x.v, ':') + 1)
                          ELSE x.v END,
                     d.key
              FROM brain_facts f
              JOIN brain_fact_kinds k ON k.id = f.kind
              CROSS JOIN LATERAL jsonb_each(COALESCE(k.body -> 'fields', '{}'::jsonb)) d(key, decl)
              CROSS JOIN LATERAL (SELECT f.fields ->> d.key AS v) x
              WHERE f.id = $1
                AND d.decl ->> 'type' = 'entity'
                AND x.v IS NOT NULL AND x.v <> ''
                AND (strpos(x.v, ':') > 0 OR d.decl ->> 'ref' IS NOT NULL)
              ON CONFLICT DO NOTHING",
            id.as_str()
        )
        .execute(conn)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn rebuild_all(conn: &mut PgConnection) -> Result<u64, RepositoryError> {
        sqlx::query!("DELETE FROM brain_fact_links")
            .execute(&mut *conn)
            .await?;
        let result = sqlx::query!(
            r"INSERT INTO brain_fact_links (fact_id, subject_kind, subject_id, via_field)
              SELECT f.id, f.subject_kind, f.subject_id, 'subject' FROM brain_facts f
              UNION
              SELECT f.id,
                     CASE WHEN strpos(x.v, ':') > 0 THEN split_part(x.v, ':', 1)
                          ELSE d.decl ->> 'ref' END,
                     CASE WHEN strpos(x.v, ':') > 0 THEN substr(x.v, strpos(x.v, ':') + 1)
                          ELSE x.v END,
                     d.key
              FROM brain_facts f
              JOIN brain_fact_kinds k ON k.id = f.kind
              CROSS JOIN LATERAL jsonb_each(COALESCE(k.body -> 'fields', '{}'::jsonb)) d(key, decl)
              CROSS JOIN LATERAL (SELECT f.fields ->> d.key AS v) x
              WHERE d.decl ->> 'type' = 'entity'
                AND x.v IS NOT NULL AND x.v <> ''
                AND (strpos(x.v, ':') > 0 OR d.decl ->> 'ref' IS NOT NULL)
              ON CONFLICT DO NOTHING"
        )
        .execute(conn)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn list_for(
        executor: impl PgExecutor<'_>,
        id: &BrainFactId,
    ) -> Result<Vec<FactLink>, RepositoryError> {
        let rows = sqlx::query!(
            "SELECT subject_kind, subject_id, via_field FROM brain_fact_links
             WHERE fact_id = $1 ORDER BY via_field, subject_kind, subject_id",
            id.as_str()
        )
        .fetch_all(executor)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| FactLink {
                subject: SubjectKey::new(row.subject_kind, row.subject_id),
                via_field: row.via_field,
            })
            .collect())
    }
}
