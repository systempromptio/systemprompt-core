//! [`ReplayRepository`]: the queries a dry run needs. Each runs on the dry
//! run's own transaction, which is always rolled back, except the read of
//! the stored facts, which runs on the read pool after it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::str::FromStr;

use sqlx::{PgConnection, PgExecutor};
use systemprompt_identifiers::{EventOutboxId, FactKindId, PipelineVersion};
use systemprompt_models::brain::FactStatus;
use systemprompt_traits::RepositoryError;

use super::diff::FactState;

struct FactStateRow {
    kind: FactKindId,
    identity_hash: String,
    statement_hash: String,
    status: String,
}

impl TryFrom<FactStateRow> for FactState {
    type Error = RepositoryError;

    fn try_from(row: FactStateRow) -> Result<Self, Self::Error> {
        Ok(Self {
            kind: row.kind,
            identity_hash: row.identity_hash,
            statement_hash: row.statement_hash,
            status: FactStatus::from_str(&row.status)
                .map_err(|e| RepositoryError::decode("status", e))?,
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ReplayRepository;

impl ReplayRepository {
    pub async fn forget_result(
        conn: &mut PgConnection,
        outbox_id: &EventOutboxId,
        version: &PipelineVersion,
    ) -> Result<(), RepositoryError> {
        sqlx::query!(
            "DELETE FROM brain_event_results WHERE outbox_id = $1 AND pipeline_version = $2",
            outbox_id.as_str(),
            version.as_str(),
        )
        .execute(conn)
        .await?;
        Ok(())
    }

    pub async fn written_in_transaction(
        conn: &mut PgConnection,
    ) -> Result<Vec<FactState>, RepositoryError> {
        sqlx::query_as!(
            FactStateRow,
            r#"SELECT kind AS "kind: FactKindId", identity_hash, statement_hash, status
               FROM brain_facts WHERE updated_at = transaction_timestamp()
               ORDER BY kind, identity_hash"#,
        )
        .fetch_all(conn)
        .await?
        .into_iter()
        .map(FactState::try_from)
        .collect()
    }

    pub async fn stored(
        executor: impl PgExecutor<'_>,
        keys: &[FactState],
    ) -> Result<Vec<FactState>, RepositoryError> {
        let kinds: Vec<String> = keys.iter().map(|k| k.kind.to_string()).collect();
        let hashes: Vec<String> = keys.iter().map(|k| k.identity_hash.clone()).collect();
        sqlx::query_as!(
            FactStateRow,
            r#"SELECT f.kind AS "kind: FactKindId", f.identity_hash, f.statement_hash, f.status
               FROM brain_facts f
               JOIN unnest($1::text[], $2::text[]) AS k(kind, identity_hash)
                 ON f.kind = k.kind AND f.identity_hash = k.identity_hash"#,
            &kinds,
            &hashes,
        )
        .fetch_all(executor)
        .await?
        .into_iter()
        .map(FactState::try_from)
        .collect()
    }
}
