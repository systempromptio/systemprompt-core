//! [`DedupeSetRepository`]: the rows one consumer round dedupes against,
//! read once for every entered candidate it holds: the fact of each
//! identity in any status, and the live facts of each kind and subject.
//! The per-candidate reads in `pipeline::dedupe` stay for what a round did
//! not preload; this one returns the same rows for a set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgConnection;
use systemprompt_identifiers::{BrainFactId, FactKindId};
use systemprompt_models::brain::{FactStatus, SubjectKey};
use systemprompt_traits::RepositoryError;

use crate::models::decode_status;
use crate::pipeline::round::{CachedFact, is_live};

#[derive(Debug, Clone, Copy, Default)]
pub struct DedupeSetRepository;

impl DedupeSetRepository {
    pub async fn list_rows(
        conn: &mut PgConnection,
        identities: &[(FactKindId, String)],
        pairs: &[(FactKindId, SubjectKey)],
        live: &[FactStatus],
    ) -> Result<Vec<CachedFact>, RepositoryError> {
        let identity_kinds: Vec<String> = identities.iter().map(|(k, _)| k.to_string()).collect();
        let hashes: Vec<String> = identities.iter().map(|(_, h)| h.clone()).collect();
        let pair_kinds: Vec<String> = pairs.iter().map(|(k, _)| k.to_string()).collect();
        let subject_kinds: Vec<String> = pairs.iter().map(|(_, s)| s.kind.clone()).collect();
        let subject_ids: Vec<String> = pairs.iter().map(|(_, s)| s.id.clone()).collect();
        let statuses: Vec<String> = live.iter().map(|s| s.as_str().to_owned()).collect();
        let rows = sqlx::query!(
            r#"SELECT id AS "id: BrainFactId", kind AS "kind: FactKindId", subject_kind,
                      subject_id, identity_hash, statement, statement_hash, status, updated_at
               FROM brain_facts f
               WHERE (f.kind, f.identity_hash) IN (
                         SELECT i.kind, i.hash
                         FROM unnest($1::text[], $2::text[]) AS i(kind, hash))
                  OR ((f.kind, f.subject_kind, f.subject_id) IN (
                         SELECT p.kind, p.subject_kind, p.subject_id
                         FROM unnest($3::text[], $4::text[], $5::text[])
                              AS p(kind, subject_kind, subject_id))
                      AND f.status = ANY($6::text[]))
               ORDER BY f.id"#,
            &identity_kinds,
            &hashes,
            &pair_kinds,
            &subject_kinds,
            &subject_ids,
            &statuses,
        )
        .fetch_all(conn)
        .await?;
        rows.into_iter()
            .map(|row| {
                Ok(CachedFact {
                    id: row.id,
                    kind: row.kind,
                    subject: SubjectKey::new(row.subject_kind, row.subject_id),
                    identity_hash: row.identity_hash,
                    statement: row.statement,
                    statement_hash: row.statement_hash,
                    live: is_live(decode_status(&row.status)?),
                    updated_at: row.updated_at,
                })
            })
            .collect()
    }
}
