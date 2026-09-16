//! Erases source keys and indirect request/invocation references before
//! dropping expired facts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::FeedbackSnapshotsRepository;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::UserId;
impl FeedbackSnapshotsRepository {
    pub(super) async fn erase_expired(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
        cutoff: DateTime<Utc>,
    ) -> crate::Result<u64> {
        sqlx::query!("DELETE FROM analytics_fact_changes c WHERE owner_id=$1 AND EXISTS(SELECT 1 FROM analytics_normalized_facts n WHERE n.owner_id=c.owner_id AND n.fact_kind=c.fact_kind AND n.source=c.source AND n.fact_id=c.fact_id AND (n.occurred_at<$2 OR EXISTS(SELECT 1 FROM analytics_normalized_facts old WHERE old.owner_id=n.owner_id AND old.occurred_at<$2 AND ((old.fact_kind='request' AND n.request_source=old.source AND n.request_id=old.fact_id) OR (old.fact_kind='invocation' AND n.invocation_source=old.source AND n.invocation_id=old.fact_id)))))",owner.as_str(),cutoff).execute(&mut **tx).await?;
        sqlx::query!("DELETE FROM analytics_fact_deltas d WHERE owner_id=$1 AND EXISTS(SELECT 1 FROM analytics_normalized_facts n WHERE n.owner_id=d.owner_id AND n.fact_kind=d.fact_kind AND n.source=d.source AND n.fact_id=d.fact_id AND (n.occurred_at<$2 OR EXISTS(SELECT 1 FROM analytics_normalized_facts old WHERE old.owner_id=n.owner_id AND old.occurred_at<$2 AND ((old.fact_kind='request' AND n.request_source=old.source AND n.request_id=old.fact_id) OR (old.fact_kind='invocation' AND n.invocation_source=old.source AND n.invocation_id=old.fact_id)))))",owner.as_str(),cutoff).execute(&mut **tx).await?;
        sqlx::query!("DELETE FROM analytics_snapshot_shadow s WHERE owner_id=$1 AND EXISTS(SELECT 1 FROM analytics_normalized_facts n WHERE n.owner_id=s.owner_id AND n.fact_kind=s.fact_kind AND n.source=s.source AND n.fact_id=s.fact_id AND (n.occurred_at<$2 OR EXISTS(SELECT 1 FROM analytics_normalized_facts old WHERE old.owner_id=n.owner_id AND old.occurred_at<$2 AND ((old.fact_kind='request' AND n.request_source=old.source AND n.request_id=old.fact_id) OR (old.fact_kind='invocation' AND n.invocation_source=old.source AND n.invocation_id=old.fact_id)))))",owner.as_str(),cutoff).execute(&mut **tx).await?;
        Ok(sqlx::query!("DELETE FROM analytics_normalized_facts n WHERE owner_id=$1 AND (n.occurred_at<$2 OR EXISTS(SELECT 1 FROM analytics_normalized_facts old WHERE old.owner_id=n.owner_id AND old.occurred_at<$2 AND ((old.fact_kind='request' AND n.request_source=old.source AND n.request_id=old.fact_id) OR (old.fact_kind='invocation' AND n.invocation_source=old.source AND n.invocation_id=old.fact_id))))",owner.as_str(),cutoff).execute(&mut **tx).await?.rows_affected())
    }
}
