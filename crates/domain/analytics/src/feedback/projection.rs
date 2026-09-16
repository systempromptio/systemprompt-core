//! Atomic fenced replacements, durable deltas and commit-ordered owner
//! generations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::columns::Columns;
use super::{ApplyOutcome, FactLease, FeedbackFactsRepository, validation};
use crate::Result;
use systemprompt_identifiers::{
    AnalyticsFactId, DeviceId, ManagedResourceId, NativeSessionId, ResourceRevisionId, UserId,
};
use systemprompt_models::feedback::analytics::{AnalyticsChange, AnalyticsChangeOperation};

struct Replacement {
    generation: i64,
    // JSON: the prior fact row is retained verbatim for delta history.
    before: Option<serde_json::Value>,
}

impl FeedbackFactsRepository {
    pub async fn apply(&self, owner: &UserId, lease: &FactLease) -> Result<ApplyOutcome> {
        let mut tx = self.pool.begin().await?;
        sqlx::query!(
            "INSERT INTO analytics_fact_checkpoints(owner_id) VALUES($1) ON CONFLICT DO NOTHING",
            owner.as_str()
        )
        .execute(&mut *tx)
        .await?;
        let checkpoint = sqlx::query!(
            "SELECT generation FROM analytics_fact_checkpoints WHERE owner_id=$1 FOR UPDATE",
            owner.as_str()
        )
        .fetch_one(&mut *tx)
        .await?;
        let row = sqlx::query!("SELECT payload FROM analytics_fact_changes WHERE owner_id=$1 AND change_id=$2 AND state='leased' AND lease_worker=$3 AND lease_epoch=$4 AND lease_until>clock_timestamp() FOR UPDATE", owner.as_str(), lease.change_id.as_str(), lease.worker_id.as_str(), lease.epoch).fetch_optional(&mut *tx).await?.ok_or_else(validation::invalid)?;
        let change: AnalyticsChange =
            serde_json::from_value(row.payload.ok_or_else(validation::invalid)?)?;
        validation::validate(&change)?;
        let kind = validation::kind(change.key.kind);
        let revision = i64::try_from(change.revision).map_err(|_error| validation::invalid())?;
        let before = sqlx::query!("SELECT revision,fact FROM analytics_normalized_facts WHERE owner_id=$1 AND fact_kind=$2 AND source=$3 AND fact_id=$4 FOR UPDATE", owner.as_str(), kind, &change.key.source, change.key.id.as_str()).fetch_optional(&mut *tx).await?;
        let replaced = before.as_ref().is_none_or(|row| row.revision < revision);
        let generation = if replaced {
            checkpoint
                .generation
                .checked_add(1)
                .ok_or_else(validation::invalid)?
        } else {
            checkpoint.generation
        };
        if replaced {
            let before_fact = before.and_then(|row| row.fact);
            self.replace_in(
                &mut tx,
                owner,
                &change,
                Replacement {
                    generation,
                    before: before_fact,
                },
            )
            .await?;
        }
        let state = if replaced { "applied" } else { "superseded" };
        let updated = sqlx::query!("UPDATE analytics_fact_changes SET state=$5,applied_at=clock_timestamp(),lease_worker=NULL,lease_until=NULL,last_error=NULL WHERE owner_id=$1 AND change_id=$2 AND state='leased' AND lease_worker=$3 AND lease_epoch=$4 AND lease_until>clock_timestamp()", owner.as_str(), lease.change_id.as_str(), lease.worker_id.as_str(), lease.epoch, state).execute(&mut *tx).await?;
        if updated.rows_affected() != 1 {
            return Err(validation::invalid());
        }
        sqlx::query!("UPDATE analytics_fact_checkpoints SET generation=$2,applied_changes=applied_changes+$3,superseded_changes=superseded_changes+$4,last_applied_at=clock_timestamp(),last_recorded_at=GREATEST(last_recorded_at,$5) WHERE owner_id=$1", owner.as_str(), generation, i64::from(replaced), i64::from(!replaced), change.recorded_at).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(ApplyOutcome {
            change_id: lease.change_id.clone(),
            generation,
            replaced,
        })
    }

    async fn replace_in(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
        change: &AnalyticsChange,
        replacement: Replacement,
    ) -> Result<()> {
        let Replacement { generation, before } = replacement;
        let fact = match &change.operation {
            AnalyticsChangeOperation::Replace { fact } => Some(fact),
            AnalyticsChangeOperation::Tombstone => None,
        };
        let columns = Columns::of(&change.key.source, fact)?;
        let after = fact.map(serde_json::to_value).transpose()?;
        let kind = validation::kind(change.key.kind);
        let revision = i64::try_from(change.revision).map_err(|_error| validation::invalid())?;
        sqlx::query!("INSERT INTO analytics_normalized_facts(owner_id,fact_kind,source,fact_id,revision,occurred_at,deleted,fact,consumer_id,device_id,host,session_id,resource_id,resource_revision_id,invocation_source,invocation_id,request_source,request_id,succeeded,currency,amount_micros,input_tokens,output_tokens,latency_micros,assessment_status,score_millionths,generation,conversation_source,conversation_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18,$19,$20,$21,$22,$23,$24,$25,$26,$27,$28,$29) ON CONFLICT(owner_id,fact_kind,source,fact_id) DO UPDATE SET revision=EXCLUDED.revision,occurred_at=EXCLUDED.occurred_at,deleted=EXCLUDED.deleted,fact=EXCLUDED.fact,consumer_id=EXCLUDED.consumer_id,device_id=EXCLUDED.device_id,host=EXCLUDED.host,session_id=EXCLUDED.session_id,resource_id=EXCLUDED.resource_id,resource_revision_id=EXCLUDED.resource_revision_id,invocation_source=EXCLUDED.invocation_source,invocation_id=EXCLUDED.invocation_id,request_source=EXCLUDED.request_source,request_id=EXCLUDED.request_id,succeeded=EXCLUDED.succeeded,currency=EXCLUDED.currency,amount_micros=EXCLUDED.amount_micros,input_tokens=EXCLUDED.input_tokens,output_tokens=EXCLUDED.output_tokens,latency_micros=EXCLUDED.latency_micros,assessment_status=EXCLUDED.assessment_status,score_millionths=EXCLUDED.score_millionths,generation=EXCLUDED.generation,conversation_source=EXCLUDED.conversation_source,conversation_id=EXCLUDED.conversation_id,updated_at=clock_timestamp()", owner.as_str(), kind, &change.key.source, change.key.id.as_str(), revision, change.occurred_at, fact.is_none(), after, columns.consumer.as_ref().map(UserId::as_str), columns.device.as_ref().map(DeviceId::as_str), columns.host, columns.session.as_ref().map(NativeSessionId::as_str), columns.resource.as_ref().map(ManagedResourceId::as_str), columns.resource_revision.as_ref().map(ResourceRevisionId::as_str), columns.invocation_source, columns.invocation.as_ref().map(AnalyticsFactId::as_str), columns.request_source, columns.request.as_ref().map(AnalyticsFactId::as_str), columns.succeeded, columns.currency, columns.amount, columns.input, columns.output, columns.latency, columns.assessment, columns.score, generation, columns.conversation_source, columns.conversation.as_ref().map(AnalyticsFactId::as_str)).execute(&mut **tx).await?;
        sqlx::query!("INSERT INTO analytics_fact_deltas(owner_id,generation,fact_kind,source,fact_id,before_fact,after_fact,occurred_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8)", owner.as_str(), generation, kind, &change.key.source, change.key.id.as_str(), before, after,change.occurred_at).execute(&mut **tx).await?;
        Ok(())
    }
}
