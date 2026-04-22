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

#[derive(Debug, Clone)]
pub struct InsertSafetyFinding<'a> {
    pub ai_request_id: &'a AiRequestId,
    pub phase: &'a str,
    pub severity: &'a str,
    pub category: &'a str,
    pub scanner: &'a str,
    pub excerpt: Option<&'a str>,
}

impl AiSafetyFindingRepository {
    pub fn new(db: &DbPool) -> Result<Self, RepositoryError> {
        let write_pool = db
            .write_pool_arc()
            .map_err(|e| RepositoryError::PoolInitialization(e.to_string()))?;
        Ok(Self { write_pool })
    }

    pub async fn insert(
        &self,
        params: InsertSafetyFinding<'_>,
    ) -> Result<AiSafetyFindingId, RepositoryError> {
        let id = AiSafetyFindingId::generate();
        sqlx::query!(
            r#"
            INSERT INTO ai_safety_findings (
                id, ai_request_id, phase, severity, category, scanner, excerpt, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, CURRENT_TIMESTAMP)
            "#,
            id.as_str(),
            params.ai_request_id.as_str(),
            params.phase,
            params.severity,
            params.category,
            params.scanner,
            params.excerpt
        )
        .execute(self.write_pool.as_ref())
        .await?;
        Ok(id)
    }
}
