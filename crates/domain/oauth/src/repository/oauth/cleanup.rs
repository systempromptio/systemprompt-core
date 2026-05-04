//! Lifecycle cleanup methods on [`OAuthRepository`] (deactivate, delete, list
//! stale).

use super::OAuthRepository;
use crate::error::OauthResult;
use chrono::Utc;
use systemprompt_identifiers::ClientId;

impl OAuthRepository {
    /// Deactivate clients flagged as inactive in the registry.
    pub async fn cleanup_inactive_clients(&self) -> OauthResult<u64> {
        self.client_repo.cleanup_inactive().await
    }

    /// Cleanup test clients older than `days_old`.
    pub async fn cleanup_old_test_clients(&self, days_old: u32) -> OauthResult<u64> {
        self.client_repo.cleanup_old_test(days_old).await
    }

    /// Delete clients that have never been used and are older than `days_old`.
    pub async fn cleanup_unused_clients(&self, days_old: u32) -> OauthResult<u64> {
        let cutoff_timestamp = Utc::now().timestamp() - (i64::from(days_old) * 24 * 60 * 60);
        self.client_repo.delete_unused(cutoff_timestamp).await
    }

    /// Delete clients last used before the `days_unused` threshold.
    pub async fn cleanup_stale_clients(&self, days_unused: u32) -> OauthResult<u64> {
        let cutoff_timestamp = Utc::now().timestamp() - (i64::from(days_unused) * 24 * 60 * 60);
        self.client_repo.delete_stale(cutoff_timestamp).await
    }

    /// List clients that have never been used and are older than `days_old`.
    pub async fn list_unused_clients(
        &self,
        days_old: u32,
    ) -> OauthResult<Vec<super::super::ClientUsageSummary>> {
        let cutoff_timestamp = Utc::now().timestamp() - (i64::from(days_old) * 24 * 60 * 60);
        self.client_repo.list_unused(cutoff_timestamp).await
    }

    /// List clients last used before the `days_unused` threshold.
    pub async fn list_stale_clients(
        &self,
        days_unused: u32,
    ) -> OauthResult<Vec<super::super::ClientUsageSummary>> {
        let cutoff_timestamp = Utc::now().timestamp() - (i64::from(days_unused) * 24 * 60 * 60);
        self.client_repo.list_stale(cutoff_timestamp).await
    }

    /// Mark test clients older than `days_old` as inactive.
    pub async fn deactivate_old_test_clients(&self, days_old: u32) -> OauthResult<u64> {
        self.client_repo.deactivate_old_test(days_old).await
    }

    /// List all clients currently flagged inactive.
    pub async fn list_inactive_clients(&self) -> OauthResult<Vec<super::super::ClientSummary>> {
        self.client_repo.list_inactive().await
    }

    /// List clients created before the `days_old` cutoff.
    pub async fn list_old_clients(
        &self,
        days_old: u32,
    ) -> OauthResult<Vec<super::super::ClientSummary>> {
        let cutoff_timestamp = Utc::now().timestamp() - (i64::from(days_old) * 24 * 60 * 60);
        self.client_repo.list_old(cutoff_timestamp).await
    }

    /// Stamp `last_used_at` on the given client to the current time.
    pub async fn update_client_last_used(&self, client_id: &ClientId) -> OauthResult<()> {
        let now = Utc::now().timestamp();
        self.client_repo.update_last_used(client_id, now).await
    }
}
