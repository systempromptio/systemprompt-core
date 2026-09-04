//! Repository for `ai_safety_findings` rows.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::RepositoryError;
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{AiRequestId, AiSafetyFindingId};

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
    // Why: "matched a block category" and "refused the call" are the same fact
    // only under `safety.mode: enforce`. Under warn they diverge, and the
    // report needs the second one, so it is stamped at insert rather than
    // re-derived later from a config that may have changed since.
    pub blocked: bool,
}

impl AiSafetyFindingRepository {
    pub fn new(db: &DbPool) -> Result<Self, RepositoryError> {
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| RepositoryError::PoolInitialization(e.to_string()))?;
        Ok(Self { write_pool })
    }

    // Why: the CLI reaches this table through a bare `PgPool` rather than a
    // `DbPool`, and the rollup is a read, so the write/read pool split this
    // type otherwise honours has nothing to enforce here.
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
