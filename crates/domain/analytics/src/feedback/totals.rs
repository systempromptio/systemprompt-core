//! Raw-reference totals deduplicate shared requests and conversation
//! assessments.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{FactsTotals, FeedbackFactsRepository, validation};
use crate::Result;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::{ManagedResourceId, UserId};

impl FeedbackFactsRepository {
    pub async fn reference_totals(
        &self,
        owner: &UserId,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
        resource: Option<&ManagedResourceId>,
    ) -> Result<FactsTotals> {
        if from >= to {
            return Err(validation::invalid());
        }
        let resource = resource.map(ManagedResourceId::as_str);
        let mut tx = self.pool.begin().await?;
        sqlx::query!("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
            .execute(&mut *tx)
            .await?;
        let invocations = sqlx::query!(r#"SELECT COUNT(*) AS "total!",COUNT(*) FILTER(WHERE resource_id IS NOT NULL) AS "verified!" FROM analytics_normalized_facts WHERE owner_id=$1 AND fact_kind='invocation' AND NOT deleted AND occurred_at>=$2 AND occurred_at<$3 AND ($4::text IS NULL OR resource_id=$4)"#, owner.as_str(), from, to, resource).fetch_one(&mut *tx).await?;
        let requests = sqlx::query!(r#"SELECT COUNT(*) AS "total!",COUNT(*) FILTER(WHERE NOT succeeded) AS "failed!",COUNT(*) FILTER(WHERE amount_micros IS NOT NULL) AS "priced!",COUNT(*) FILTER(WHERE latency_micros IS NOT NULL) AS "measured!",COUNT(*) FILTER(WHERE input_tokens IS NOT NULL AND output_tokens IS NOT NULL) AS "tokens_measured!" FROM analytics_normalized_facts r WHERE owner_id=$1 AND fact_kind='request' AND NOT deleted AND occurred_at>=$2 AND occurred_at<$3 AND ($4::text IS NULL OR EXISTS(SELECT 1 FROM analytics_normalized_facts a WHERE a.owner_id=r.owner_id AND a.fact_kind='resource_association' AND NOT a.deleted AND a.resource_id=$4 AND a.request_source=r.source AND a.request_id=r.fact_id))"#, owner.as_str(), from, to, resource).fetch_one(&mut *tx).await?;
        let spend = sqlx::query!(r#"SELECT currency AS "currency!",SUM(amount_micros)::text AS "amount!" FROM analytics_normalized_facts r WHERE owner_id=$1 AND fact_kind='request' AND NOT deleted AND occurred_at>=$2 AND occurred_at<$3 AND currency IS NOT NULL AND ($4::text IS NULL OR EXISTS(SELECT 1 FROM analytics_normalized_facts a WHERE a.owner_id=r.owner_id AND a.fact_kind='resource_association' AND NOT a.deleted AND a.resource_id=$4 AND a.request_source=r.source AND a.request_id=r.fact_id)) GROUP BY currency"#, owner.as_str(), from, to, resource).fetch_all(&mut *tx).await?;
        let assessments = sqlx::query!(r#"SELECT COUNT(DISTINCT (conversation_source,conversation_id)) AS "total!",COUNT(DISTINCT (conversation_source,conversation_id)) FILTER(WHERE assessment_status='scored') AS "scored!",COUNT(DISTINCT (conversation_source,conversation_id)) FILTER(WHERE assessment_status='failed') AS "failed!" FROM (SELECT DISTINCT ON (conversation_source,conversation_id) * FROM analytics_normalized_facts WHERE owner_id=$1 AND fact_kind='assessment' AND NOT deleted ORDER BY conversation_source,conversation_id,occurred_at DESC,source,fact_id) a WHERE occurred_at>=$2 AND occurred_at<$3 AND ($4::text IS NULL OR EXISTS(SELECT 1 FROM analytics_normalized_facts i WHERE i.owner_id=a.owner_id AND i.fact_kind='invocation' AND NOT i.deleted AND i.resource_id=$4 AND i.source=a.invocation_source AND i.fact_id=a.invocation_id))"#, owner.as_str(), from, to, resource).fetch_one(&mut *tx).await?;
        let mut totals = FactsTotals {
            invocations: invocations.total,
            verified_invocations: invocations.verified,
            requests: requests.total,
            failed_requests: requests.failed,
            priced_requests: requests.priced,
            latency_measured_requests: requests.measured,
            token_measured_requests: requests.tokens_measured,
            assessed_conversations: assessments.scored,
            assessment_conversations: assessments.total,
            failed_assessments: assessments.failed,
            related_spend_non_additive: resource.is_some(),
            ..FactsTotals::default()
        };
        for row in spend {
            totals.spend_by_currency.insert(
                row.currency,
                row.amount.parse().map_err(|_error| validation::invalid())?,
            );
        }
        tx.commit().await?;
        Ok(totals)
    }
}
