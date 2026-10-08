//! Archiving a user instead of deleting them (NFR-5.2).
//!
//! An archive keeps the `users` row and everything that keys on it: the row
//! moves to status `deleted` — which every listing, search and sign-in path
//! already excludes — and records who archived it, when and why. The same
//! transaction revokes the user's sessions, API keys and device certificates,
//! so nothing issued before the archive authenticates after it. A restore
//! inside the retention window reverses the status and the archive fields;
//! revoked credentials stay revoked and are re-issued by signing in again.
//! Physical removal is the purge in `updates.rs`, run for an archive past the
//! window by `database_cleanup` and refused while `legal_hold` is set.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use systemprompt_identifiers::UserId;

use crate::error::{Result, UserError};
use crate::models::UserStatus;
use crate::repository::UserRepository;

/// Who archives a user, why, and whether the archive is under legal hold.
#[derive(Debug, Clone, Copy, Default)]
pub struct ArchiveParams<'a> {
    pub archived_by: Option<&'a UserId>,
    pub reason: Option<&'a str>,
    pub legal_hold: bool,
}

/// The credentials an archive revoked.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ArchiveOutcome {
    pub sessions: u64,
    pub api_keys: u64,
    pub device_certs: u64,
}

/// A user's archive fields. `archived_at` is `None` for a row deleted before
/// archiving existed, which is restorable but never purged automatically.
#[derive(Debug, Clone)]
pub struct ArchiveState {
    pub id: UserId,
    pub status: String,
    pub archived_at: Option<DateTime<Utc>>,
    pub archived_by: Option<String>,
    pub archive_reason: Option<String>,
    pub legal_hold: bool,
}

impl ArchiveState {
    #[must_use]
    pub fn is_archived(&self) -> bool {
        self.status == UserStatus::Deleted.as_str()
    }
}

impl UserRepository {
    pub async fn archive(&self, id: &UserId, params: ArchiveParams<'_>) -> Result<ArchiveOutcome> {
        let mut tx = self.write_pool.begin().await?;
        let archived = sqlx::query!(
            r#"
            UPDATE users
            SET status = $2,
                archived_at = COALESCE(archived_at, NOW()),
                archived_by = COALESCE(archived_by, $3),
                archive_reason = COALESCE(archive_reason, $4),
                legal_hold = legal_hold OR $5,
                updated_at = NOW()
            WHERE id = $1
            "#,
            id.as_str(),
            UserStatus::Deleted.as_str(),
            params.archived_by.map(UserId::as_str),
            params.reason,
            params.legal_hold
        )
        .execute(&mut *tx)
        .await?;
        if archived.rows_affected() == 0 {
            return Err(UserError::NotFound(id.clone()));
        }
        let sessions = sqlx::query!(
            "UPDATE user_sessions SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL",
            id.as_str()
        )
        .execute(&mut *tx)
        .await?;
        let api_keys = sqlx::query!(
            "UPDATE user_api_keys SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL",
            id.as_str()
        )
        .execute(&mut *tx)
        .await?;
        let device_certs = sqlx::query!(
            "UPDATE user_device_certs SET revoked_at = NOW() WHERE user_id = $1 AND revoked_at IS NULL",
            id.as_str()
        )
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(ArchiveOutcome {
            sessions: sessions.rows_affected(),
            api_keys: api_keys.rows_affected(),
            device_certs: device_certs.rows_affected(),
        })
    }

    pub async fn restore(&self, id: &UserId, window_days: u32) -> Result<bool> {
        let restored = sqlx::query!(
            r#"
            UPDATE users
            SET status = $2, archived_at = NULL, archived_by = NULL,
                archive_reason = NULL, updated_at = NOW()
            WHERE id = $1 AND status = $3
              AND (archived_at IS NULL
                   OR archived_at > NOW() - make_interval(days => $4::int))
            "#,
            id.as_str(),
            UserStatus::Active.as_str(),
            UserStatus::Deleted.as_str(),
            i32::try_from(window_days).unwrap_or(i32::MAX)
        )
        .execute(&*self.write_pool)
        .await?;
        Ok(restored.rows_affected() > 0)
    }

    pub async fn set_legal_hold(&self, id: &UserId, hold: bool) -> Result<()> {
        let updated = sqlx::query!(
            "UPDATE users SET legal_hold = $2, updated_at = NOW() WHERE id = $1",
            id.as_str(),
            hold
        )
        .execute(&*self.write_pool)
        .await?;
        if updated.rows_affected() == 0 {
            return Err(UserError::NotFound(id.clone()));
        }
        Ok(())
    }

    pub async fn find_archive_state(&self, id: &UserId) -> Result<Option<ArchiveState>> {
        let row = sqlx::query!(
            r#"
            SELECT id, status, archived_at, archived_by, archive_reason, legal_hold
            FROM users WHERE id = $1
            "#,
            id.as_str()
        )
        .fetch_optional(&*self.pool)
        .await?;
        Ok(row.map(|r| ArchiveState {
            id: UserId::new(r.id),
            status: r.status,
            archived_at: r.archived_at,
            archived_by: r.archived_by,
            archive_reason: r.archive_reason,
            legal_hold: r.legal_hold,
        }))
    }

    pub async fn list_purgeable_archives(
        &self,
        window_days: u32,
        limit: i64,
    ) -> Result<Vec<UserId>> {
        let ids = sqlx::query_scalar!(
            r#"
            SELECT id FROM users
            WHERE status = $1 AND NOT legal_hold AND archived_at IS NOT NULL
              AND archived_at < NOW() - make_interval(days => $2::int)
            ORDER BY archived_at
            LIMIT $3
            "#,
            UserStatus::Deleted.as_str(),
            i32::try_from(window_days).unwrap_or(i32::MAX),
            limit
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(ids.into_iter().map(UserId::new).collect())
    }
}
