//! Fenced, append-only execution progress with idempotent delivery.
//!
//! Events report progress; they do not grant approvals or authorize write
//! replay.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::{ExecutionLease, WorkerRecord};
use crate::Result;
use crate::experiments::{conflict, content_digest, invalid};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::types::Json;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStage {
    Provisioning,
    Context,
    Specification,
    Publication,
    Verification,
    Cleanup,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionEvent {
    pub sequence: i64,
    pub stage: ExecutionStage,
    pub summary: String,
}

impl ExecutionEvent {
    pub const fn builder(sequence: i64, stage: ExecutionStage) -> ExecutionEventBuilder {
        ExecutionEventBuilder {
            sequence,
            stage,
            summary: None,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if !(0..1000).contains(&self.sequence)
            || self.summary.trim().is_empty()
            || self.summary.len() > 8192
        {
            return Err(invalid(
                "Events require sequence 0–999 and a summary of 1–8192 bytes",
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ExecutionEventBuilder {
    sequence: i64,
    stage: ExecutionStage,
    summary: Option<String>,
}

impl ExecutionEventBuilder {
    pub fn summary(mut self, value: String) -> Self {
        self.summary = Some(value);
        self
    }

    pub fn build(self) -> Result<ExecutionEvent> {
        let event = ExecutionEvent {
            sequence: self.sequence,
            stage: self.stage,
            summary: self
                .summary
                .ok_or_else(|| invalid("Event summary required"))?,
        };
        event.validate()?;
        Ok(event)
    }
}

#[derive(Debug, Clone)]
pub struct ExecutionEventRepository {
    pool: PgPool,
}

impl ExecutionEventRepository {
    pub const fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn append(
        &self,
        worker: &WorkerRecord,
        lease: &ExecutionLease,
        event: &ExecutionEvent,
    ) -> Result<()> {
        event.validate()?;
        let mut tx = self.pool.begin().await?;
        super::lock_owner(&mut tx, &worker.owner_id).await?;
        let live = sqlx::query_scalar!(
            "SELECT x.id FROM eval_executions x JOIN eval_experiments e ON e.id=x.experiment_id JOIN eval_workers w ON w.id=x.lease_owner AND w.owner_id=e.owner_id WHERE e.owner_id=$1 AND x.id=$2 AND x.lease_owner=$3 AND x.fencing_token=$4 AND w.id=$5 AND w.environment=$6 AND w.enabled AND w.expires_at>NOW() AND e.status='running' AND x.status='running' AND x.lease_expires_at>NOW() AND x.deadline_at>NOW() FOR UPDATE OF x",
            worker.owner_id.as_str(), lease.execution_id.as_str(), lease.worker_id.as_str(), lease.fencing_token, worker.id.as_str(), worker.environment
        ).fetch_optional(&mut *tx).await?;
        if live.is_none() {
            return Err(conflict("Event requires a live, owned worker lease"));
        }
        let digest = content_digest(event)?;
        let existing = sqlx::query_scalar!(
            "SELECT digest FROM eval_execution_events WHERE execution_id=$1 AND sequence=$2",
            lease.execution_id.as_str(),
            event.sequence
        )
        .fetch_optional(&mut *tx)
        .await?;
        if let Some(existing) = existing {
            if existing != digest {
                return Err(conflict("Event sequence already has different content"));
            }
            tx.commit().await?;
            return Ok(());
        }
        let next = sqlx::query_scalar!(
            "SELECT COALESCE(MAX(sequence)+1,0) FROM eval_execution_events WHERE execution_id=$1",
            lease.execution_id.as_str()
        )
        .fetch_one(&mut *tx)
        .await?
        .unwrap_or(0);
        if event.sequence != next {
            return Err(conflict("Events must be delivered in sequence"));
        }
        sqlx::query!(
            "INSERT INTO eval_execution_events(execution_id,sequence,payload,digest) VALUES($1,$2,$3,$4)",
            lease.execution_id.as_str(), event.sequence, Json(event) as _, digest
        ).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }
}
