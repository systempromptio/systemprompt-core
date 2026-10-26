//! `config_plane_base` and `config_conflicts` access for the explicit apply
//! (`config apply`, `/admin/sync`) and for `cloud sync export`.
//!
//! Every write takes the apply's own transaction, so the base an apply
//! records, the conflicts it detects and the rows it changed commit together
//! or not at all.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::Serialize;

use super::config_reconcile::{ConfigReconcileRepository, PlaneTransaction};
use crate::error::SchedulerResult;

/// One entity's base as the apply records it.
#[derive(Debug, Clone, Copy)]
pub struct BaseRecord<'a> {
    pub plane: &'a str,
    pub entity: &'a str,
    pub declared_hash: &'a str,
    pub source: &'a str,
    pub applied_by: &'a str,
}

/// Both sides of an entity the file and the database each changed.
#[derive(Debug, Clone, Copy)]
pub struct ConflictRecord<'a> {
    pub plane: &'a str,
    pub entity: &'a str,
    pub base_hash: Option<&'a str>,
    pub file_hash: &'a str,
    pub file_value: Option<&'a str>,
    pub db_hash: &'a str,
    pub db_value: Option<&'a str>,
}

/// The plane-level record of one explicit apply.
#[derive(Debug, Clone, Copy)]
pub struct LastApply<'a> {
    pub plane: &'a str,
    pub authority: &'a str,
    pub declared_hash: &'a str,
    pub actor: &'a str,
    pub mode: &'a str,
    pub drift_rows: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ConflictRow {
    pub id: i64,
    pub plane: String,
    pub entity: String,
    pub base_hash: Option<String>,
    pub file_hash: String,
    pub file_value: Option<String>,
    pub db_hash: String,
    pub db_value: Option<String>,
    pub detected_at: DateTime<Utc>,
}

impl ConfigReconcileRepository {
    pub async fn find_plane_applied(
        &self,
        tx: &mut PlaneTransaction,
        plane: &str,
    ) -> SchedulerResult<bool> {
        let applied = sqlx::query_scalar!(
            r#"SELECT (EXISTS (SELECT 1 FROM config_plane_base WHERE plane = $1)
                    OR EXISTS (SELECT 1 FROM config_plane_state
                                WHERE plane = $1 AND last_apply_at IS NOT NULL)) AS "applied!""#,
            plane,
        )
        .fetch_one(&mut **tx)
        .await?;
        Ok(applied)
    }

    pub async fn list_base(
        &self,
        tx: &mut PlaneTransaction,
        plane: &str,
    ) -> SchedulerResult<BTreeMap<String, String>> {
        let rows = sqlx::query!(
            "SELECT entity, declared_hash FROM config_plane_base WHERE plane = $1",
            plane,
        )
        .fetch_all(&mut **tx)
        .await?;
        Ok(rows
            .into_iter()
            .map(|r| (r.entity, r.declared_hash))
            .collect())
    }

    pub async fn upsert_base(
        &self,
        tx: &mut PlaneTransaction,
        record: &BaseRecord<'_>,
    ) -> SchedulerResult<()> {
        sqlx::query!(
            r"INSERT INTO config_plane_base (plane, entity, declared_hash, source, applied_by)
              VALUES ($1, $2, $3, $4, $5)
              ON CONFLICT (plane, entity) DO UPDATE
                 SET declared_hash = EXCLUDED.declared_hash,
                     source = EXCLUDED.source,
                     applied_by = EXCLUDED.applied_by,
                     applied_at = NOW()",
            record.plane,
            record.entity,
            record.declared_hash,
            record.source,
            record.applied_by,
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn delete_base(
        &self,
        tx: &mut PlaneTransaction,
        plane: &str,
        entity: &str,
    ) -> SchedulerResult<()> {
        sqlx::query!(
            "DELETE FROM config_plane_base WHERE plane = $1 AND entity = $2",
            plane,
            entity,
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn upsert_conflict(
        &self,
        tx: &mut PlaneTransaction,
        record: &ConflictRecord<'_>,
    ) -> SchedulerResult<()> {
        sqlx::query!(
            r"INSERT INTO config_conflicts
                  (plane, entity, base_hash, file_hash, file_value, db_hash, db_value)
              VALUES ($1, $2, $3, $4, $5, $6, $7)
              ON CONFLICT (plane, entity) WHERE resolved_at IS NULL DO UPDATE
                 SET base_hash = EXCLUDED.base_hash,
                     file_hash = EXCLUDED.file_hash,
                     file_value = EXCLUDED.file_value,
                     db_hash = EXCLUDED.db_hash,
                     db_value = EXCLUDED.db_value,
                     detected_at = NOW()",
            record.plane,
            record.entity,
            record.base_hash,
            record.file_hash,
            record.file_value,
            record.db_hash,
            record.db_value,
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn update_conflict_converged(
        &self,
        tx: &mut PlaneTransaction,
        plane: &str,
        entity: &str,
        actor: &str,
    ) -> SchedulerResult<()> {
        sqlx::query!(
            r"UPDATE config_conflicts
                 SET resolved_by = $3, resolved_at = NOW(), resolution = 'edited'
               WHERE plane = $1 AND entity = $2 AND resolved_at IS NULL",
            plane,
            entity,
            actor,
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn set_last_apply(
        &self,
        tx: &mut PlaneTransaction,
        apply: &LastApply<'_>,
    ) -> SchedulerResult<()> {
        let outcome = if apply.drift_rows == 0 {
            "in_sync"
        } else {
            "drift_reported"
        };
        sqlx::query!(
            r"INSERT INTO config_plane_state
                  (plane, authority, declared_hash, drift_rows, outcome,
                   last_apply_declared_hash, last_apply_actor, last_apply_mode, last_apply_at)
              VALUES ($1, $2, $3, $4, $5, $3, $6, $7, NOW())
              ON CONFLICT (plane) DO UPDATE
                 SET declared_hash = EXCLUDED.declared_hash,
                     drift_rows = EXCLUDED.drift_rows,
                     outcome = EXCLUDED.outcome,
                     last_apply_declared_hash = EXCLUDED.last_apply_declared_hash,
                     last_apply_actor = EXCLUDED.last_apply_actor,
                     last_apply_mode = EXCLUDED.last_apply_mode,
                     last_apply_at = EXCLUDED.last_apply_at",
            apply.plane,
            apply.authority,
            apply.declared_hash,
            apply.drift_rows,
            outcome,
            apply.actor,
            apply.mode,
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub async fn list_open_conflicts(
        &self,
        plane: Option<&str>,
    ) -> SchedulerResult<Vec<ConflictRow>> {
        Ok(sqlx::query_as!(
            ConflictRow,
            r"SELECT id, plane, entity, base_hash, file_hash, file_value, db_hash, db_value,
                     detected_at
                FROM config_conflicts
               WHERE resolved_at IS NULL AND ($1::TEXT IS NULL OR plane = $1)
               ORDER BY plane, entity",
            plane,
        )
        .fetch_all(&**self.read_pool())
        .await?)
    }
}
