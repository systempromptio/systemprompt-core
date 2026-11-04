//! The alias rows a fact owns, and the facts that could take them over:
//! the two set statements behind [`crate::index::release_alias_owners`].
//! A contributor is a live fact of the same subject kind whose subject id
//! (as written or with `-`/`_` as spaces), subject display name or any
//! field value normalises to a released alias; the exact derivation
//! ([`crate::index::derive_aliases`]) is applied in Rust afterwards, so
//! this pre-filter may admit a fact the reindex then declines, never the
//! reverse.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_identifiers::BrainFactId;
use systemprompt_traits::RepositoryError;

use super::AliasRepository;

/// One alias an owner released: the pair the next contributor must match.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleasedAlias {
    pub subject_kind: String,
    pub alias_norm: String,
}

// Why: Postgres — this list mirrors the four statuses the dedupe stage
// treats as live; a stale, superseded or retired fact names nothing.
const CONTRIBUTING_STATUSES: [&str; 4] = ["candidate", "proposed", "confirmed", "disputed"];

impl AliasRepository {
    pub async fn delete_owned(
        conn: &mut PgConnection,
        owners: &[BrainFactId],
    ) -> Result<Vec<ReleasedAlias>, RepositoryError> {
        let owners: Vec<String> = owners.iter().map(ToString::to_string).collect();
        let rows = sqlx::query!(
            "DELETE FROM brain_aliases WHERE owner_fact_id = ANY($1::text[])
             RETURNING subject_kind, alias_norm",
            &owners,
        )
        .fetch_all(conn)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| ReleasedAlias {
                subject_kind: row.subject_kind,
                alias_norm: row.alias_norm,
            })
            .collect())
    }

    pub async fn list_contributors(
        conn: &mut PgConnection,
        released: &[ReleasedAlias],
    ) -> Result<Vec<BrainFactId>, RepositoryError> {
        let kinds: Vec<String> = released.iter().map(|r| r.subject_kind.clone()).collect();
        let norms: Vec<String> = released.iter().map(|r| r.alias_norm.clone()).collect();
        let statuses: Vec<String> = CONTRIBUTING_STATUSES
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        Ok(sqlx::query_scalar!(
            r#"SELECT DISTINCT f.id AS "id!: BrainFactId"
               FROM unnest($1::text[], $2::text[]) AS r(kind, norm)
               JOIN brain_facts f ON f.subject_kind = r.kind
               LEFT JOIN brain_subjects s
                      ON s.subject_kind = f.subject_kind AND s.subject_id = f.subject_id
               WHERE f.status = ANY($3::text[])
                 AND (lower(regexp_replace(btrim(f.subject_id), '\s+', ' ', 'g')) = r.norm
                   OR lower(regexp_replace(btrim(translate(f.subject_id, '-_', '  ')),
                                           '\s+', ' ', 'g')) = r.norm
                   OR lower(regexp_replace(btrim(s.display_name), '\s+', ' ', 'g')) = r.norm
                   OR EXISTS (SELECT 1 FROM jsonb_each_text(f.fields) AS kv
                              WHERE lower(regexp_replace(btrim(kv.value), '\s+', ' ', 'g'))
                                    = r.norm))
               ORDER BY 1"#,
            &kinds,
            &norms,
            &statuses,
        )
        .fetch_all(conn)
        .await?)
    }
}
