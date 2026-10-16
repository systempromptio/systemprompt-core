//! `config_plane_state` and `config_change_audit` access for the config
//! reconciler, `/readyz` and `admin config planes`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use systemprompt_database::DbPool;

use crate::error::SchedulerResult;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PlaneStateRow {
    pub plane: String,
    pub authority: String,
    pub declared_hash: Option<String>,
    pub drift_rows: i32,
    pub outcome: String,
    pub last_error: Option<String>,
    pub reconciled_at: DateTime<Utc>,
    pub corrected_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct PlaneStateUpdate<'a> {
    pub plane: &'a str,
    pub authority: &'a str,
    pub declared_hash: Option<&'a str>,
    pub drift_rows: i32,
    pub outcome: &'a str,
    pub last_error: Option<&'a str>,
    pub corrected: bool,
}

#[derive(Debug, Clone)]
pub struct ConfigChangeRecord<'a> {
    pub plane: &'a str,
    pub entity: &'a str,
    pub change_kind: &'a str,
    pub before_value: Option<&'a str>,
    pub after_value: Option<&'a str>,
    pub mode: &'a str,
    pub actor: &'a str,
    pub reason: &'a str,
    pub declared_hash: &'a str,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConfigChangeRow {
    pub id: i64,
    pub plane: String,
    pub entity: String,
    pub change_kind: String,
    pub before_value: Option<String>,
    pub after_value: Option<String>,
    pub mode: String,
    pub actor: String,
    pub reason: String,
    pub declared_hash: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct ConfigReconcileRepository {
    pool: Arc<PgPool>,
    write_pool: Arc<PgPool>,
}

impl ConfigReconcileRepository {
    pub fn new(db: &DbPool) -> Self {
        Self {
            pool: db.pool(),
            write_pool: db.write_pool(),
        }
    }

    pub async fn upsert_state(&self, update: &PlaneStateUpdate<'_>) -> SchedulerResult<()> {
        sqlx::query!(
            r"INSERT INTO config_plane_state
                  (plane, authority, declared_hash, drift_rows, outcome, last_error,
                   reconciled_at, corrected_at)
              VALUES ($1, $2, $3, $4, $5, $6, NOW(), CASE WHEN $7 THEN NOW() END)
              ON CONFLICT (plane) DO UPDATE
                 SET authority = EXCLUDED.authority,
                     declared_hash = EXCLUDED.declared_hash,
                     drift_rows = EXCLUDED.drift_rows,
                     outcome = EXCLUDED.outcome,
                     last_error = EXCLUDED.last_error,
                     reconciled_at = NOW(),
                     corrected_at = COALESCE(EXCLUDED.corrected_at, config_plane_state.corrected_at)",
            update.plane,
            update.authority,
            update.declared_hash,
            update.drift_rows,
            update.outcome,
            update.last_error,
            update.corrected,
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(())
    }

    pub async fn list_states(&self) -> SchedulerResult<Vec<PlaneStateRow>> {
        Ok(sqlx::query_as!(
            PlaneStateRow,
            r"SELECT plane, authority, declared_hash, drift_rows, outcome, last_error,
                     reconciled_at, corrected_at
                FROM config_plane_state
               ORDER BY plane"
        )
        .fetch_all(&*self.pool)
        .await?)
    }

    pub async fn record_changes(&self, records: &[ConfigChangeRecord<'_>]) -> SchedulerResult<()> {
        let mut tx = self.write_pool.begin().await?;
        for record in records {
            sqlx::query!(
                r"INSERT INTO config_change_audit
                      (plane, entity, change_kind, before_value, after_value, mode, actor,
                       reason, declared_hash)
                  VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                record.plane,
                record.entity,
                record.change_kind,
                record.before_value,
                record.after_value,
                record.mode,
                record.actor,
                record.reason,
                record.declared_hash,
            )
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_changes(
        &self,
        plane: &str,
        limit: i64,
    ) -> SchedulerResult<Vec<ConfigChangeRow>> {
        Ok(sqlx::query_as!(
            ConfigChangeRow,
            r"SELECT id, plane, entity, change_kind, before_value, after_value, mode, actor,
                     reason, declared_hash, created_at
                FROM config_change_audit
               WHERE plane = $1
               ORDER BY id DESC
               LIMIT $2",
            plane,
            limit,
        )
        .fetch_all(&*self.pool)
        .await?)
    }
}
