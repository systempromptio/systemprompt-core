//! Scanner findings recorded against an artifact (`mcp_artifact_findings`).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;
use systemprompt_identifiers::ArtifactId;

/// The ingestion phase every artifact finding is raised in.
pub const PHASE_TOOL_RESULT: &str = "tool_result";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactFinding {
    pub phase: &'static str,
    pub severity: String,
    pub category: String,
    pub scanner: String,
    pub path: Option<String>,
    pub excerpt: Option<String>,
    pub redacted: bool,
}

#[derive(Debug, Clone)]
pub struct ArtifactFindingRecord {
    pub id: uuid::Uuid,
    pub artifact_id: ArtifactId,
    pub phase: String,
    pub severity: String,
    pub category: String,
    pub scanner: String,
    pub path: Option<String>,
    pub excerpt: Option<String>,
    pub redacted: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct ArtifactFindingRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl ArtifactFindingRepository {
    pub fn new(db: &DbPool) -> McpDomainResult<Self> {
        let pool = db.pool_arc().map_err(|e| {
            crate::error::McpDomainError::Internal(format!("Database must be PostgreSQL: {e}"))
        })?;
        let write_pool = db.write_pool_arc().map_err(|e| {
            crate::error::McpDomainError::Internal(format!("Database must be PostgreSQL: {e}"))
        })?;
        Ok(Self { pool, write_pool })
    }

    pub async fn insert_findings(
        &self,
        artifact_id: &ArtifactId,
        findings: &[ArtifactFinding],
    ) -> McpDomainResult<()> {
        for finding in findings {
            sqlx::query!(
                r#"
                INSERT INTO mcp_artifact_findings (
                    artifact_id, phase, severity, category, scanner, path, excerpt, redacted
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
                artifact_id.as_str(),
                finding.phase,
                finding.severity,
                finding.category,
                finding.scanner,
                finding.path.as_deref(),
                finding.excerpt.as_deref(),
                finding.redacted
            )
            .execute(&*self.write_pool)
            .await?;
        }
        Ok(())
    }

    pub async fn list_for_artifact(
        &self,
        artifact_id: &ArtifactId,
    ) -> McpDomainResult<Vec<ArtifactFindingRecord>> {
        Ok(sqlx::query_as!(
            ArtifactFindingRecord,
            r#"
            SELECT id as "id!", artifact_id as "artifact_id!: ArtifactId", phase as "phase!",
                   severity as "severity!", category as "category!", scanner as "scanner!",
                   path, excerpt, redacted as "redacted!", created_at as "created_at!"
            FROM mcp_artifact_findings
            WHERE artifact_id = $1
            ORDER BY created_at
            "#,
            artifact_id.as_str()
        )
        .fetch_all(&*self.pool)
        .await?)
    }
}
