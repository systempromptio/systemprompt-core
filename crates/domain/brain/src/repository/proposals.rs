//! [`ProposalRepository`]: proposals analyses draft, and the person's
//! decision on each; nothing here applies a proposal.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;
use systemprompt_identifiers::{BrainProposalId, UserId};
use systemprompt_models::brain::{BrainProposal, ProposalKind, ProposalStatus};
use systemprompt_traits::RepositoryError;

use super::Pools;
use crate::models::NewProposal;

struct ProposalRow {
    id: BrainProposalId,
    analysis: String,
    kind: String,
    status: String,
    summary: String,
    // JSON: the body column stores the analysis's typed draft.
    body: serde_json::Value,
    created_at: DateTime<Utc>,
    decided_by: Option<UserId>,
    decided_at: Option<DateTime<Utc>>,
}

impl TryFrom<ProposalRow> for BrainProposal {
    type Error = RepositoryError;

    fn try_from(row: ProposalRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            analysis: row.analysis,
            kind: decode_enum::<ProposalKind>("kind", row.kind)?,
            status: decode_enum::<ProposalStatus>("status", row.status)?,
            summary: row.summary,
            body: row.body,
            created_at: row.created_at,
            decided_by: row.decided_by,
            decided_at: row.decided_at,
        })
    }
}

fn decode_enum<T: serde::de::DeserializeOwned>(
    field: &'static str,
    value: String,
) -> Result<T, RepositoryError> {
    serde_json::from_value(serde_json::Value::String(value))
        .map_err(|e| RepositoryError::decode(field, e))
}

fn encode_enum<T: serde::Serialize>(value: T) -> Result<String, RepositoryError> {
    match serde_json::to_value(value)? {
        serde_json::Value::String(text) => Ok(text),
        _ => Err(RepositoryError::invalid_argument(
            "enum",
            "not a unit variant",
        )),
    }
}

#[derive(Debug, Clone)]
pub struct ProposalRepository {
    pools: Pools,
}

impl ProposalRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn insert(
        executor: impl PgExecutor<'_>,
        proposal: &NewProposal,
    ) -> Result<BrainProposalId, RepositoryError> {
        let id = BrainProposalId::generate();
        sqlx::query!(
            "INSERT INTO brain_proposals (id, analysis, kind, summary, body)
             VALUES ($1, $2, $3, $4, $5)",
            id.as_str(),
            proposal.analysis,
            encode_enum(proposal.kind)?,
            proposal.summary,
            proposal.body,
        )
        .execute(executor)
        .await?;
        Ok(id)
    }

    pub async fn decide(
        executor: impl PgExecutor<'_>,
        id: &BrainProposalId,
        status: ProposalStatus,
        by: &UserId,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query!(
            "UPDATE brain_proposals SET status = $2, decided_by = $3, decided_at = now()
             WHERE id = $1 AND status = 'open'",
            id.as_str(),
            encode_enum(status)?,
            by.as_str(),
        )
        .execute(executor)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    // JSON: the replay column stores the serialised `ReplayReport`.
    pub async fn attach_replay(
        executor: impl PgExecutor<'_>,
        id: &BrainProposalId,
        report: &serde_json::Value,
    ) -> Result<bool, RepositoryError> {
        let result = sqlx::query!(
            "UPDATE brain_proposals SET replay = $2 WHERE id = $1",
            id.as_str(),
            report,
        )
        .execute(executor)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn exists(
        executor: impl PgExecutor<'_>,
        id: &BrainProposalId,
    ) -> Result<bool, RepositoryError> {
        let found = sqlx::query_scalar!(
            r#"SELECT EXISTS (SELECT 1 FROM brain_proposals WHERE id = $1) AS "found!""#,
            id.as_str(),
        )
        .fetch_one(executor)
        .await?;
        Ok(found)
    }

    pub async fn list_open(&self, limit: i64) -> Result<Vec<BrainProposal>, RepositoryError> {
        sqlx::query_as!(
            ProposalRow,
            r#"SELECT id AS "id: BrainProposalId", analysis, kind, status, summary, body,
                      created_at, decided_by AS "decided_by: UserId", decided_at
               FROM brain_proposals WHERE status = 'open'
               ORDER BY created_at DESC, id LIMIT $1"#,
            limit,
        )
        .fetch_all(&*self.pools.read)
        .await?
        .into_iter()
        .map(BrainProposal::try_from)
        .collect()
    }
}
