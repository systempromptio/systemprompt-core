//! Archive, restore, legal hold and the guarded purge (NFR-5.2).
//!
//! `archive` is what deleting a user means: the account stops signing in and
//! leaves every roster, its credentials are revoked, and its history stays.
//! `purge` is the physical delete, allowed only for an archived account that
//! is not under legal hold; `database_cleanup` calls it for archives past the
//! retention window. [`UserService::merge_users`] is unchanged: a merge still
//! deletes its source once every row has moved.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::UserId;

use super::UserService;
use crate::error::{Result, UserError};
use crate::repository::{ArchiveOutcome, ArchiveParams, ArchiveState, PurgeCount};

impl UserService {
    pub async fn archive(&self, id: &UserId, params: ArchiveParams<'_>) -> Result<ArchiveOutcome> {
        self.repository.archive(id, params).await
    }

    pub async fn restore(&self, id: &UserId, window_days: u32) -> Result<()> {
        if self.repository.restore(id, window_days).await? {
            return Ok(());
        }
        match self.repository.find_archive_state(id).await? {
            None => Err(UserError::NotFound(id.clone())),
            Some(_) => Err(UserError::RestoreRefused {
                id: id.clone(),
                window_days,
            }),
        }
    }

    pub async fn set_legal_hold(&self, id: &UserId, hold: bool) -> Result<()> {
        self.repository.set_legal_hold(id, hold).await
    }

    pub async fn find_archive_state(&self, id: &UserId) -> Result<Option<ArchiveState>> {
        self.repository.find_archive_state(id).await
    }

    /// Physically deletes an archived user and every row keyed on them.
    pub async fn purge(&self, id: &UserId) -> Result<Vec<PurgeCount>> {
        let state = self
            .repository
            .find_archive_state(id)
            .await?
            .ok_or_else(|| UserError::NotFound(id.clone()))?;
        if !state.is_archived() {
            return Err(UserError::NotArchived(id.clone()));
        }
        if state.legal_hold {
            return Err(UserError::LegalHold(id.clone()));
        }
        self.repository.delete(id).await
    }

    /// Purges up to `limit` archives older than `window_days` that are not
    /// under legal hold. Returns the ids purged.
    pub async fn purge_expired_archives(&self, window_days: u32, limit: i64) -> Result<Vec<UserId>> {
        let ids = self
            .repository
            .list_purgeable_archives(window_days, limit)
            .await?;
        let mut purged = Vec::with_capacity(ids.len());
        for id in ids {
            self.purge(&id).await?;
            purged.push(id);
        }
        Ok(purged)
    }

    pub async fn list_purgeable_archives(&self, window_days: u32, limit: i64) -> Result<Vec<UserId>> {
        self.repository
            .list_purgeable_archives(window_days, limit)
            .await
    }
}
