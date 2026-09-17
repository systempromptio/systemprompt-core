//! Content-addressed storage for artifact bodies (`artifact_payloads`).
//!
//! A body is keyed by the SHA-256 of its canonical JSON and stored once;
//! artifacts reference it by digest and `ref_count` tracks how many do. The
//! digest doubles as the "already scanned" cache: a body that exists has been
//! through the scanners once and is not scanned again.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::McpDomainResult;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use std::sync::Arc;
use systemprompt_database::DbPool;

#[derive(Debug, Clone)]
pub struct ArtifactPayloadRecord {
    pub sha256: String,
    pub byte_len: i32,
    // JSON: the typed artifact body, whose shape is fixed by the owning
    // artifact's `artifact_type`.
    pub body: serde_json::Value,
    pub ref_count: i32,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct ArtifactPayloadRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl ArtifactPayloadRepository {
    pub fn new(db: &DbPool) -> McpDomainResult<Self> {
        let pool = db.pool_arc().map_err(|e| {
            crate::error::McpDomainError::Internal(format!("Database must be PostgreSQL: {e}"))
        })?;
        let write_pool = db.write_pool_arc().map_err(|e| {
            crate::error::McpDomainError::Internal(format!("Database must be PostgreSQL: {e}"))
        })?;
        Ok(Self { pool, write_pool })
    }

    /// Stores a body under its digest, or bumps the reference count and
    /// last-seen time of the one already there. Returns whether it was new.
    pub async fn upsert_payload(
        &self,
        sha256: &str,
        byte_len: i32,
        // JSON: typed artifact body, already scanned and redacted.
        body: &serde_json::Value,
    ) -> McpDomainResult<bool> {
        let created = sqlx::query_scalar!(
            r#"
            INSERT INTO artifact_payloads (sha256, byte_len, body, ref_count)
            VALUES ($1, $2, $3, 1)
            ON CONFLICT (sha256) DO UPDATE SET
                ref_count = artifact_payloads.ref_count + 1,
                last_seen_at = CURRENT_TIMESTAMP
            RETURNING (xmax = 0) as "created!"
            "#,
            sha256,
            byte_len,
            body
        )
        .fetch_one(&*self.write_pool)
        .await?;
        Ok(created)
    }

    pub async fn find_payload(
        &self,
        sha256: &str,
    ) -> McpDomainResult<Option<ArtifactPayloadRecord>> {
        Ok(sqlx::query_as!(
            ArtifactPayloadRecord,
            r#"
            SELECT sha256 as "sha256!", byte_len as "byte_len!", body as "body!",
                   ref_count as "ref_count!", first_seen_at as "first_seen_at!",
                   last_seen_at as "last_seen_at!"
            FROM artifact_payloads
            WHERE sha256 = $1
            "#,
            sha256
        )
        .fetch_optional(&*self.pool)
        .await?)
    }

    pub async fn payload_exists(&self, sha256: &str) -> McpDomainResult<bool> {
        Ok(sqlx::query_scalar!(
            r#"SELECT EXISTS(SELECT 1 FROM artifact_payloads WHERE sha256 = $1) as "exists!""#,
            sha256
        )
        .fetch_one(&*self.pool)
        .await?)
    }

    /// Drops one reference; the body is deleted when nothing references it.
    pub async fn release_payload(&self, sha256: &str) -> McpDomainResult<()> {
        sqlx::query!(
            r#"
            UPDATE artifact_payloads
            SET ref_count = GREATEST(ref_count - 1, 0)
            WHERE sha256 = $1
            "#,
            sha256
        )
        .execute(&*self.write_pool)
        .await?;
        sqlx::query!(
            r#"DELETE FROM artifact_payloads WHERE sha256 = $1 AND ref_count <= 0"#,
            sha256
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }
}
