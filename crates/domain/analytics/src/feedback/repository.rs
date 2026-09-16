//! Transactional independent change admission and owner-scoped fact reads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ChangeReceipt, StoredFact, validation};
use crate::Result;
use sqlx::PgPool;
use systemprompt_identifiers::UserId;
use systemprompt_models::feedback::ContentDigest;
use systemprompt_models::feedback::analytics::{AnalyticsChange, AnalyticsFactKey};

#[derive(Debug, Clone)]
pub struct FeedbackFactsRepository {
    pub(super) pool: PgPool,
}

impl FeedbackFactsRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn submit(&self, owner: &UserId, change: &AnalyticsChange) -> Result<ChangeReceipt> {
        let mut tx = self.pool.begin().await?;
        let receipt = Self::submit_in(&mut tx, owner, change).await?;
        tx.commit().await?;
        Ok(receipt)
    }

    pub async fn submit_in(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
        change: &AnalyticsChange,
    ) -> Result<ChangeReceipt> {
        validation::validate(change)?;
        sqlx::query!(
            "INSERT INTO analytics_fact_checkpoints(owner_id) VALUES($1) ON CONFLICT DO NOTHING",
            owner.as_str()
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query!(
            "SELECT generation FROM analytics_fact_checkpoints WHERE owner_id=$1 FOR UPDATE",
            owner.as_str()
        )
        .fetch_one(&mut **tx)
        .await?;
        let kind = validation::kind(change.key.kind);
        let revision = i64::try_from(change.revision).map_err(|_error| validation::invalid())?;
        let digest = ContentDigest::of(&serde_json::to_vec(&(
            &change.key,
            change.revision,
            change.occurred_at,
            &change.operation,
        ))?);
        let payload = serde_json::to_value(change)?;
        sqlx::query!("INSERT INTO analytics_fact_changes(owner_id,change_id,fact_kind,source,fact_id,revision,occurred_at,recorded_at,payload,payload_digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) ON CONFLICT DO NOTHING", owner.as_str(), change.change_id.as_str(), kind, &change.key.source, change.key.id.as_str(), revision, change.occurred_at, change.recorded_at, payload, digest.as_str()).execute(&mut **tx).await?;
        let rows = sqlx::query!("SELECT change_id,state,payload_digest FROM analytics_fact_changes WHERE owner_id=$1 AND (change_id=$2 OR (fact_kind=$3 AND source=$4 AND fact_id=$5 AND revision=$6))", owner.as_str(), change.change_id.as_str(), kind, &change.key.source, change.key.id.as_str(), revision).fetch_all(&mut **tx).await?;
        if rows.len() != 1 || rows[0].payload_digest != digest.as_str() {
            return Err(validation::invalid());
        }
        Ok(ChangeReceipt {
            change_id: systemprompt_identifiers::AnalyticsChangeId::new(rows[0].change_id.clone()),
            state: validation::parse_state(&rows[0].state)?,
        })
    }

    pub async fn get_fact(
        &self,
        owner: &UserId,
        key: &AnalyticsFactKey,
    ) -> Result<Option<StoredFact>> {
        let kind = validation::kind(key.kind);
        let row = sqlx::query!("SELECT revision,occurred_at,fact,generation FROM analytics_normalized_facts WHERE owner_id=$1 AND fact_kind=$2 AND source=$3 AND fact_id=$4", owner.as_str(), kind, &key.source, key.id.as_str()).fetch_optional(&self.pool).await?;
        row.map(|row| {
            Ok(StoredFact {
                key: key.clone(),
                revision: row.revision,
                occurred_at: row.occurred_at,
                fact: row.fact.map(serde_json::from_value).transpose()?,
                generation: row.generation,
            })
        })
        .transpose()
    }
    pub async fn list_facts(
        &self,
        owner: &UserId,
        after: Option<&AnalyticsFactKey>,
        limit: u32,
    ) -> Result<Vec<StoredFact>> {
        if !(1..=256).contains(&limit) {
            return Err(validation::invalid());
        }
        let after_kind = after.map(|key| validation::kind(key.kind));
        let after_source = after.map(|key| key.source.as_str());
        let after_id = after.map(|key| key.id.as_str());
        let limit = i64::from(limit);
        let rows = sqlx::query!("SELECT fact_kind,source,fact_id,revision,occurred_at,fact,generation FROM analytics_normalized_facts WHERE owner_id=$1 AND ($2::text IS NULL OR (fact_kind,source,fact_id)>($2,$3,$4)) ORDER BY fact_kind,source,fact_id LIMIT $5", owner.as_str(), after_kind, after_source, after_id, limit).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|row| {
                Ok(StoredFact {
                    key: AnalyticsFactKey {
                        kind: validation::parse_kind(&row.fact_kind)?,
                        source: row.source,
                        id: systemprompt_identifiers::AnalyticsFactId::new(row.fact_id),
                    },
                    revision: row.revision,
                    occurred_at: row.occurred_at,
                    fact: row.fact.map(serde_json::from_value).transpose()?,
                    generation: row.generation,
                })
            })
            .collect()
    }

    pub async fn change_status(
        &self,
        owner: &UserId,
        id: &systemprompt_identifiers::AnalyticsChangeId,
    ) -> Result<Option<super::ChangeDiagnostic>> {
        let row = sqlx::query!("SELECT state,attempts,lease_until,last_error FROM analytics_fact_changes WHERE owner_id=$1 AND change_id=$2", owner.as_str(), id.as_str()).fetch_optional(&self.pool).await?;
        row.map(|row| {
            Ok(super::ChangeDiagnostic {
                change_id: id.clone(),
                state: validation::parse_state(&row.state)?,
                attempts: row.attempts,
                lease_until: row.lease_until,
                last_error: row.last_error,
            })
        })
        .transpose()
    }
}
