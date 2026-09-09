//! Environment-bound worker credentials with explicit owner and revocation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::Result;
use crate::experiments::{invalid, missing};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use sqlx::types::Json;
use systemprompt_identifiers::{EvalWorkerId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerRecord {
    pub id: EvalWorkerId,
    pub owner_id: UserId,
    pub environment: String,
    pub name: String,
    pub enabled: bool,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

pub struct WorkerCredential {
    pub id: EvalWorkerId,
    token: String,
}

impl std::fmt::Debug for WorkerCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerCredential")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

impl WorkerCredential {
    pub fn expose_token(&self) -> &str {
        &self.token
    }
}

#[derive(Debug, Clone)]
pub struct WorkerRepository {
    pool: PgPool,
}

impl WorkerRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(
        &self,
        owner: &UserId,
        environment: &str,
        name: &str,
    ) -> Result<WorkerCredential> {
        if environment.trim().is_empty()
            || environment.len() > 512
            || name.trim().is_empty()
            || name.len() > 128
        {
            return Err(invalid("Worker environment and name are required"));
        }
        let id = EvalWorkerId::generate();
        let token = format!(
            "speval_{}.{}",
            EvalWorkerId::generate(),
            EvalWorkerId::generate()
        );
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
        sqlx::query!("INSERT INTO eval_workers(id,owner_id,environment,name,token_hash) VALUES($1,$2,$3,$4,$5)",
            id.as_str(), owner.as_str(), environment, name, token_hash).execute(&self.pool).await?;
        Ok(WorkerCredential { id, token })
    }

    pub async fn authenticate(&self, token: &str, environment: &str) -> Result<WorkerRecord> {
        if token.len() > 128 || !token.starts_with("speval_") {
            return Err(missing("Worker credential unavailable"));
        }
        let token_hash = hex::encode(Sha256::digest(token.as_bytes()));
        Ok(sqlx::query_scalar!(
            r#"SELECT to_jsonb(w) - 'token_hash' AS "record!: Json<WorkerRecord>" FROM eval_workers w WHERE token_hash=$1 AND environment=$2 AND enabled=TRUE AND expires_at>NOW()"#,
            token_hash, environment
        ).fetch_optional(&self.pool).await?.ok_or_else(|| missing("Worker credential unavailable"))?.0)
    }

    pub async fn revoke(&self, owner: &UserId, worker: &EvalWorkerId) -> Result<()> {
        let changed = sqlx::query!(
            "UPDATE eval_workers SET enabled=FALSE WHERE id=$1 AND owner_id=$2",
            worker.as_str(),
            owner.as_str()
        )
        .execute(&self.pool)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(missing("Worker unavailable in this scope"));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct WorkerRecordBuilder {
    record: WorkerRecord,
}

impl WorkerRecord {
    pub fn builder(id: EvalWorkerId, owner_id: UserId) -> WorkerRecordBuilder {
        WorkerRecordBuilder {
            record: Self {
                id,
                owner_id,
                environment: String::new(),
                name: String::new(),
                enabled: true,
                expires_at: chrono::Utc::now() + chrono::Duration::days(7),
            },
        }
    }
}

impl WorkerRecordBuilder {
    pub fn environment(mut self, value: String) -> Self {
        self.record.environment = value;
        self
    }
    pub fn name(mut self, value: String) -> Self {
        self.record.name = value;
        self
    }
    pub const fn enabled(mut self, value: bool) -> Self {
        self.record.enabled = value;
        self
    }
    pub const fn expires_at(mut self, value: chrono::DateTime<chrono::Utc>) -> Self {
        self.record.expires_at = value;
        self
    }
    pub fn build(self) -> Result<WorkerRecord> {
        if self.record.environment.trim().is_empty() || self.record.name.trim().is_empty() {
            return Err(invalid("Worker environment and name required"));
        }
        Ok(self.record)
    }
}
