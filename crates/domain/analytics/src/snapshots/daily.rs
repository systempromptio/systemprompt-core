//! Transactional replacement of only affected UTC-day contributions.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::FeedbackSnapshotsRepository;
use chrono::NaiveDate;
use systemprompt_identifiers::UserId;

#[derive(Debug, Clone, Copy)]
pub(super) struct FactReference<'a> {
    pub kind: &'a str,
    pub source: &'a str,
    pub id: &'a str,
    // JSON: retained fact payload; the query reads its keys with jsonb operators.
    pub fact: Option<&'a serde_json::Value>,
}

impl FeedbackSnapshotsRepository {
    pub(super) async fn rebuild_days(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
        days: &[NaiveDate],
        generation: i64,
    ) -> crate::Result<()> {
        sqlx::query!("INSERT INTO analytics_snapshot_dirty(owner_id,scope) SELECT owner_id,scope FROM analytics_snapshot_daily WHERE owner_id=$1 AND day=ANY($2) UNION SELECT owner_id,scope FROM analytics_snapshot_contributions WHERE owner_id=$1 AND day=ANY($2) ON CONFLICT DO NOTHING",owner.as_str(),days).execute(&mut **tx).await?;
        sqlx::query!("DELETE FROM analytics_snapshot_daily WHERE owner_id=$1 AND day=ANY($2) AND NOT suppressed",owner.as_str(),days).execute(&mut **tx).await?;
        sqlx::query!(
            "DELETE FROM analytics_snapshot_identities WHERE owner_id=$1 AND day=ANY($2)",
            owner.as_str(),
            days
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query!(r#"INSERT INTO analytics_snapshot_daily(owner_id,scope,day,metrics,spend,histogram,cohort,generation)
   WITH contributions AS MATERIALIZED(SELECT * FROM analytics_snapshot_contributions WHERE owner_id=$1 AND day=ANY($2)),
   currencies AS(SELECT scope,day,currency,SUM(amount)::text AS amount FROM contributions WHERE fact_kind='request' AND currency IS NOT NULL GROUP BY scope,day,currency),
   spend AS(SELECT scope,day,jsonb_object_agg(currency,amount) AS value FROM currencies GROUP BY scope,day),
   buckets AS(SELECT scope,day,CASE WHEN latency=0 THEN 0 ELSE 65-position('1' in (latency::bit(64))::text) END AS bucket,COUNT(*) AS count FROM contributions WHERE fact_kind='request' AND latency IS NOT NULL GROUP BY scope,day,bucket),
   histograms AS(SELECT scope,day,jsonb_object_agg(bucket::text,count) AS value FROM buckets GROUP BY scope,day)
   SELECT $1,c.scope,c.day,jsonb_build_object(
    'invocations',COUNT(*) FILTER(WHERE fact_kind='invocation'),
    'verified_invocations',COUNT(*) FILTER(WHERE fact_kind='invocation' AND resource_id IS NOT NULL),
    'requests',COUNT(*) FILTER(WHERE fact_kind='request'),'failed_requests',COUNT(*) FILTER(WHERE fact_kind='request' AND NOT succeeded),
    'priced_requests',COUNT(*) FILTER(WHERE fact_kind='request' AND amount IS NOT NULL),
    'latency_measured_requests',COUNT(*) FILTER(WHERE fact_kind='request' AND latency IS NOT NULL),
    'token_measured_requests',COUNT(*) FILTER(WHERE fact_kind='request' AND input_tokens IS NOT NULL AND output_tokens IS NOT NULL),
    'input_tokens',COALESCE(SUM(input_tokens) FILTER(WHERE fact_kind='request'),0)::bigint,
    'output_tokens',COALESCE(SUM(output_tokens) FILTER(WHERE fact_kind='request'),0)::bigint,
    'assessed_conversations',COUNT(*) FILTER(WHERE fact_kind='assessment' AND assessment_status='scored'),
    'assessment_conversations',COUNT(*) FILTER(WHERE fact_kind='assessment'),
    'failed_assessments',COUNT(*) FILTER(WHERE fact_kind='assessment' AND assessment_status='failed')),
    COALESCE(s.value,'{}'::jsonb),COALESCE(h.value,'{}'::jsonb),COUNT(DISTINCT consumer_id),$3
   FROM contributions c LEFT JOIN spend s ON s.scope=c.scope AND s.day=c.day LEFT JOIN histograms h ON h.scope=c.scope AND h.day=c.day
   GROUP BY c.scope,c.day,s.value,h.value ON CONFLICT(owner_id,scope,day) DO NOTHING"#,owner.as_str(),days,generation).execute(&mut **tx).await?;
        sqlx::query!("INSERT INTO analytics_snapshot_identities(owner_id,scope,day,kind,identity) SELECT DISTINCT owner_id,scope,day,'user',consumer_id FROM analytics_snapshot_contributions WHERE owner_id=$1 AND day=ANY($2) AND consumer_id IS NOT NULL UNION SELECT DISTINCT owner_id,scope,day,'session',session_identity FROM analytics_snapshot_contributions WHERE owner_id=$1 AND day=ANY($2) AND session_identity IS NOT NULL ON CONFLICT DO NOTHING",owner.as_str(),days).execute(&mut **tx).await?;
        Ok(())
    }
    pub(super) async fn affected_days(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        owner: &UserId,
        fact: &FactReference<'_>,
    ) -> crate::Result<Vec<NaiveDate>> {
        let FactReference {
            kind,
            source,
            id,
            fact,
        } = *fact;
        Ok(sqlx::query_scalar!(r#"SELECT DISTINCT day AS "day!" FROM analytics_snapshot_dimensions d WHERE owner_id=$1 AND (
   (fact_kind=$2 AND source=$3 AND fact_id=$4) OR
   ($2='invocation' AND fact_kind='assessment' AND invocation_source=$3 AND invocation_id=$4) OR
   ($2='resource_association' AND fact_kind='request' AND source=$5::jsonb->'value'->'request_key'->>'source' AND fact_id=$5::jsonb->'value'->'request_key'->>'id') OR
   ($2='assessment' AND fact_kind='assessment' AND conversation_source=$5::jsonb->'value'->'conversation_key'->>'source' AND conversation_id=$5::jsonb->'value'->'conversation_key'->>'id'))"#,owner.as_str(),kind,source,id,fact).fetch_all(&mut **tx).await?)
    }
}
