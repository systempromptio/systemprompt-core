//! Range assembly merges additive days and counts identities across the entire
//! interval.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{
    FeedbackSnapshot, FeedbackSnapshotsRepository, LatencyHistogram, SnapshotMetrics, invalid,
};
use chrono::{DateTime, NaiveDate, Utc};
use std::collections::BTreeMap;
use systemprompt_identifiers::{ManagedResourceId, UserId};

impl SnapshotMetrics {
    pub(super) fn add(&mut self, other: &Self) -> crate::Result<()> {
        macro_rules! add {($($field:ident),*)=>{$(self.$field=self.$field.checked_add(other.$field).ok_or_else(||invalid("Snapshot total overflow"))?;)*};}
        add!(
            invocations,
            verified_invocations,
            requests,
            failed_requests,
            priced_requests,
            latency_measured_requests,
            token_measured_requests,
            input_tokens,
            output_tokens,
            assessed_conversations,
            assessment_conversations,
            failed_assessments
        );
        Ok(())
    }
}

pub(super) struct Range<'a> {
    pub(super) scope: &'a str,
    pub(super) from: NaiveDate,
    pub(super) to: NaiveDate,
    pub(super) generation: i64,
    pub(super) fact_generation: i64,
    pub(super) now: DateTime<Utc>,
}

impl FeedbackSnapshotsRepository {
    pub(super) async fn assemble(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
        range: &Range<'_>,
    ) -> crate::Result<FeedbackSnapshot> {
        let rows=sqlx::query!("SELECT metrics,spend,histogram,histogram_version,suppressed FROM analytics_snapshot_daily WHERE owner_id=$1 AND scope=$2 AND day>=$3 AND day<$4 ORDER BY day LIMIT 365",owner.as_str(),range.scope,range.from,range.to).fetch_all(&mut **tx).await?;
        let mut metrics = SnapshotMetrics::default();
        let mut spend: BTreeMap<String, i128> = BTreeMap::new();
        let mut histogram = LatencyHistogram::default();
        let mut suppressed = 0i64;
        for row in rows {
            if row.suppressed {
                suppressed += 1;
                continue;
            }
            metrics.add(&serde_json::from_value(row.metrics)?)?;
            let values: BTreeMap<String, String> = serde_json::from_value(row.spend)?;
            for (currency, amount) in values {
                let amount: i128 = amount
                    .parse()
                    .map_err(|_error| invalid("Invalid recorded spend"))?;
                let value = spend.entry(currency).or_default();
                *value = value
                    .checked_add(amount)
                    .ok_or_else(|| invalid("Spend overflow"))?;
            }
            histogram.merge(&LatencyHistogram {
                version: u32::try_from(row.histogram_version)
                    .map_err(|_error| invalid("Invalid histogram version"))?,
                buckets: serde_json::from_value(row.histogram)?,
            })?;
        }
        let identity_available = range.from >= range.now.date_naive() - chrono::Duration::days(89);
        let identities = if identity_available {
            Some(sqlx::query!(r#"SELECT COUNT(DISTINCT identity) FILTER(WHERE kind='user') AS "users!",COUNT(DISTINCT identity) FILTER(WHERE kind='session') AS "sessions!" FROM analytics_snapshot_identities WHERE owner_id=$1 AND scope=$2 AND day>=$3 AND day<$4"#,owner.as_str(),range.scope,range.from,range.to).fetch_one(&mut **tx).await?)
        } else {
            None
        };
        Ok(FeedbackSnapshot {
            resource_id: (!range.scope.is_empty()).then(|| ManagedResourceId::new(range.scope)),
            generation: range.generation,
            fact_generation: range.fact_generation,
            from_day: range.from,
            to_day: range.to,
            generated_at: range.now,
            metrics,
            spend_by_currency: spend,
            distinct_users: identities.as_ref().map(|value| value.users),
            distinct_sessions: identities.map(|value| value.sessions),
            histogram,
            related_spend_non_additive: !range.scope.is_empty(),
            suppressed_days: suppressed,
            historical_identity_available: identity_available,
        })
    }
}
