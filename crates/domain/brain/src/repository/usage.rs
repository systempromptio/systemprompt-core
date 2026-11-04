//! [`UsageRepository`]: the per-fact injection and citation counts the
//! `brain_usage` job reads over the results window, and the one write that
//! sets the smoothed rates on `brain_facts`.
//!
//! [`UsageRepository::set_rates`] is the one exception to the rule that
//! [`super::FactRepository::apply`] alone changes a stored fact: the usage
//! columns are a derived measurement, not a statement about the world, so
//! the write records no revision and leaves `revision` and `updated_at`
//! untouched. Only `usage_cited_rate`, `usage_ignored_rate` and
//! `usage_updated_at` move.
//!
//! An injection counts once per recall row that was not skipped and spent
//! tokens, so a same-turn reuse (tokens `0`) is not a second injection.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use sqlx::PgExecutor;
use systemprompt_identifiers::BrainFactId;
use systemprompt_models::brain::CITED_IN_RESPONSE;
use systemprompt_traits::RepositoryError;

use super::Pools;

/// How often one fact was injected and cited inside the window.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UsageCounts {
    pub fact_id: BrainFactId,
    pub injected: u64,
    pub cited: u64,
}

/// The rates one fact is set to.
#[derive(Debug, Clone, PartialEq)]
pub struct FactUsageRates {
    pub fact_id: BrainFactId,
    pub cited_rate: f64,
    pub ignored_rate: f64,
}

#[derive(Debug, Clone)]
pub struct UsageRepository {
    pools: Pools,
}

impl UsageRepository {
    pub(crate) const fn new(pools: Pools) -> Self {
        Self { pools }
    }

    pub async fn list_counts_since(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<UsageCounts>, RepositoryError> {
        let rows = sqlx::query!(
            r#"WITH injected AS (
                   SELECT elem->>'id' AS fact_id, count(*) AS n
                   FROM brain_recalls r, jsonb_array_elements(r.injected) AS elem
                   WHERE r.created_at >= $1 AND r.skipped IS NULL AND r.tokens > 0
                   GROUP BY 1),
               cited AS (
                   SELECT o.fact_id, count(*) AS n
                   FROM brain_outcomes o
                   JOIN brain_recalls r ON r.id = o.recall_id
                   WHERE o.signal = $2 AND r.created_at >= $1
                     AND r.skipped IS NULL AND r.tokens > 0
                   GROUP BY 1)
               SELECT f.id AS "id: BrainFactId",
                      COALESCE(i.n, 0)::bigint AS "injected!",
                      COALESCE(c.n, 0)::bigint AS "cited!"
               FROM brain_facts f
               LEFT JOIN injected i ON i.fact_id = f.id
               LEFT JOIN cited c ON c.fact_id = f.id
               ORDER BY f.id"#,
            since,
            CITED_IN_RESPONSE,
        )
        .fetch_all(&*self.pools.read)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| UsageCounts {
                fact_id: row.id,
                injected: u64::try_from(row.injected).unwrap_or(0),
                cited: u64::try_from(row.cited).unwrap_or(0),
            })
            .collect())
    }

    pub async fn set_rates(
        executor: impl PgExecutor<'_>,
        rates: &[FactUsageRates],
    ) -> Result<u64, RepositoryError> {
        if rates.is_empty() {
            return Ok(0);
        }
        let ids: Vec<String> = rates
            .iter()
            .map(|r| r.fact_id.as_str().to_owned())
            .collect();
        let cited: Vec<f64> = rates.iter().map(|r| r.cited_rate).collect();
        let ignored: Vec<f64> = rates.iter().map(|r| r.ignored_rate).collect();
        let result = sqlx::query!(
            "UPDATE brain_facts f
             SET usage_cited_rate = u.cited,
                 usage_ignored_rate = u.ignored,
                 usage_updated_at = now()
             FROM unnest($1::text[], $2::float8[], $3::float8[]) AS u(id, cited, ignored)
             WHERE f.id = u.id AND f.id = ANY($1)",
            &ids,
            &cited,
            &ignored,
        )
        .execute(executor)
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn set_rates_detached(
        &self,
        rates: &[FactUsageRates],
    ) -> Result<u64, RepositoryError> {
        Self::set_rates(&*self.pools.write, rates).await
    }
}
