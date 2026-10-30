//! [`AnalysisRepository`]: the read-only aggregates the core analyses draft
//! proposals from, each over one history window.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt_extension::brain::AnalysisWindow;
use systemprompt_identifiers::FactKindId;
use systemprompt_models::brain::CITED_IN_RESPONSE;
use systemprompt_traits::RepositoryError;

/// Live facts of one kind, how often recall injected them in the window and
/// how often an injection was cited.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindUsage {
    pub kind: FactKindId,
    pub facts: i64,
    pub injections: i64,
    pub citations: i64,
}

/// Recalls one client made in the window and how many injected nothing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientRecalls {
    pub client: String,
    pub recalls: i64,
    pub empty: i64,
}

#[derive(Debug, Clone)]
pub struct AnalysisRepository {
    pool: Arc<PgPool>,
}

impl AnalysisRepository {
    #[must_use]
    pub const fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }

    pub async fn kind_usage(
        &self,
        window: AnalysisWindow,
    ) -> Result<Vec<KindUsage>, RepositoryError> {
        Ok(sqlx::query_as!(
            KindUsage,
            r#"WITH live AS (
                   SELECT kind, count(*) AS n FROM brain_facts
                   WHERE status IN ('candidate', 'proposed', 'confirmed')
                   GROUP BY kind
               ), injected AS (
                   SELECT f.kind, count(*) AS n
                   FROM brain_recalls r
                   CROSS JOIN LATERAL jsonb_array_elements(r.injected) AS e(item)
                   JOIN brain_facts f ON f.id = e.item->>'id'
                   WHERE r.created_at >= $1 AND r.created_at < $2
                   GROUP BY f.kind
               ), cited AS (
                   SELECT f.kind, count(*) AS n
                   FROM brain_outcomes o JOIN brain_facts f ON f.id = o.fact_id
                   WHERE o.signal = $3 AND o.value > 0
                     AND o.observed_at >= $1 AND o.observed_at < $2
                   GROUP BY f.kind
               ), kinds AS (
                   SELECT kind FROM live UNION SELECT kind FROM injected
                   UNION SELECT kind FROM cited
               )
               SELECT k.kind AS "kind!: FactKindId",
                      COALESCE(live.n, 0) AS "facts!",
                      COALESCE(injected.n, 0) AS "injections!",
                      COALESCE(cited.n, 0) AS "citations!"
               FROM kinds k
               LEFT JOIN live ON live.kind = k.kind
               LEFT JOIN injected ON injected.kind = k.kind
               LEFT JOIN cited ON cited.kind = k.kind
               ORDER BY k.kind"#,
            window.since,
            window.until,
            CITED_IN_RESPONSE,
        )
        .fetch_all(&*self.pool)
        .await?)
    }

    pub async fn client_recalls(
        &self,
        window: AnalysisWindow,
    ) -> Result<Vec<ClientRecalls>, RepositoryError> {
        Ok(sqlx::query_as!(
            ClientRecalls,
            r#"SELECT client,
                      count(*) AS "recalls!",
                      count(*) FILTER (WHERE jsonb_array_length(injected) = 0) AS "empty!"
               FROM brain_recalls
               WHERE created_at >= $1 AND created_at < $2 AND skipped IS NULL
               GROUP BY client ORDER BY client"#,
            window.since,
            window.until,
        )
        .fetch_all(&*self.pool)
        .await?)
    }
}
