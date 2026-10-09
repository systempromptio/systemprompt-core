//! Repository for `ai_safety_findings` rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiRequestId, AiSafetyFindingId};
use systemprompt_traits::RepositoryError;

#[must_use]
#[derive(Debug, Clone)]
pub struct AiSafetyFindingRepository {
    write_pool: Arc<PgPool>,
}

/// One row of the safety-findings rollup: how often a category fired and how
/// often it actually refused a call.
///
/// The two counts diverge under `safety.mode: warn`, which is what makes the
/// row worth reading — a category with a high count and a zero blocked count
/// is a block list entry waiting to be reconsidered.
#[derive(Debug, Clone)]
pub struct SafetyFindingRollupRow {
    pub category: String,
    pub scanner: String,
    pub severity: String,
    pub phase: String,
    pub count: i64,
    pub blocked_count: i64,
    pub last_seen: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone)]
pub struct InsertSafetyFinding<'a> {
    pub ai_request_id: &'a AiRequestId,
    pub phase: &'a str,
    pub severity: &'a str,
    pub category: &'a str,
    pub scanner: &'a str,
    pub excerpt: Option<&'a str>,
    pub blocked: bool,
}

impl AiSafetyFindingRepository {
    pub fn new(db: &DbPool) -> Self {
        let write_pool = db.write_pool();
        Self { write_pool }
    }

    pub const fn from_pool(pool: Arc<PgPool>) -> Self {
        Self { write_pool: pool }
    }

    pub async fn insert(
        &self,
        params: InsertSafetyFinding<'_>,
    ) -> Result<AiSafetyFindingId, RepositoryError> {
        let id = AiSafetyFindingId::generate();
        sqlx::query!(
            r#"
            INSERT INTO ai_safety_findings (
                id, ai_request_id, phase, severity, category, scanner, excerpt, blocked, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, CURRENT_TIMESTAMP)
            "#,
            id.as_str(),
            params.ai_request_id.as_str(),
            params.phase,
            params.severity,
            params.category,
            params.scanner,
            params.excerpt,
            params.blocked
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(id)
    }

    pub async fn insert_many(
        &self,
        findings: &[InsertSafetyFinding<'_>],
    ) -> Result<u64, RepositoryError> {
        if findings.is_empty() {
            return Ok(0);
        }
        let mut ids = Vec::with_capacity(findings.len());
        let mut request_ids = Vec::with_capacity(findings.len());
        let mut phases = Vec::with_capacity(findings.len());
        let mut severities = Vec::with_capacity(findings.len());
        let mut categories = Vec::with_capacity(findings.len());
        let mut scanners = Vec::with_capacity(findings.len());
        let mut excerpts: Vec<Option<String>> = Vec::with_capacity(findings.len());
        let mut blocked = Vec::with_capacity(findings.len());
        for f in findings {
            ids.push(AiSafetyFindingId::generate().as_str().to_owned());
            request_ids.push(f.ai_request_id.as_str().to_owned());
            phases.push(f.phase.to_owned());
            severities.push(f.severity.to_owned());
            categories.push(f.category.to_owned());
            scanners.push(f.scanner.to_owned());
            excerpts.push(f.excerpt.map(str::to_owned));
            blocked.push(f.blocked);
        }
        let result = sqlx::query!(
            r#"
            INSERT INTO ai_safety_findings (
                id, ai_request_id, phase, severity, category, scanner, excerpt, blocked, created_at
            )
            SELECT t.id, t.ai_request_id, t.phase, t.severity, t.category, t.scanner, t.excerpt,
                   t.blocked, CURRENT_TIMESTAMP
            FROM UNNEST(
                $1::text[], $2::text[], $3::text[], $4::text[], $5::text[], $6::text[],
                $7::text[], $8::bool[]
            ) AS t(id, ai_request_id, phase, severity, category, scanner, excerpt, blocked)
            "#,
            &ids,
            &request_ids,
            &phases,
            &severities,
            &categories,
            &scanners,
            &excerpts,
            &blocked
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(result.rows_affected())
    }

    pub async fn list_rollup(
        &self,
        since: Option<chrono::DateTime<chrono::Utc>>,
        limit: i64,
    ) -> Result<Vec<SafetyFindingRollupRow>, RepositoryError> {
        let rows = sqlx::query_as!(
            SafetyFindingRollupRow,
            r#"
            SELECT category AS "category!", scanner AS "scanner!", severity AS "severity!",
                   phase AS "phase!", COUNT(*) AS "count!",
                   COUNT(*) FILTER (WHERE blocked) AS "blocked_count!",
                   MAX(created_at) AS "last_seen!"
            FROM ai_safety_findings
            WHERE ($1::timestamptz IS NULL OR created_at >= $1)
            GROUP BY category, scanner, severity, phase
            ORDER BY COUNT(*) DESC, MAX(created_at) DESC
            LIMIT $2
            "#,
            since,
            limit
        )
        .fetch_all(self.write_pool.as_ref())
        .await?;
        Ok(rows)
    }
}
