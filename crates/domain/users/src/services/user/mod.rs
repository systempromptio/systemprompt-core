//! User account service.
//!
//! [`UserService`] is the primary entry point for the users domain, delegating
//! to [`UserRepository`] for lookups, listing and search, session management,
//! account creation (including anonymous and federated identities), field
//! updates, bulk operations and statistics.
//!
//! Account merging spans every domain that keys rows on a user. The service
//! runs each injected [`OwnerReassignment`](systemprompt_traits::OwnerReassignment)
//! — one per owning crate, each in its own transaction and re-runnable — and
//! only then the users-owned step that moves sessions, records the merge and
//! deletes the source. A failed reassignment stops the merge with the source
//! still present, so a rerun completes it. A service built without
//! reassignments refuses to merge rather than delete a user whose rows it
//! cannot move.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod provider;

use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use systemprompt_identifiers::{SessionId, UserId};
use systemprompt_traits::DynOwnerReassignment;

use crate::error::{Result, UserError};
use crate::models::{
    User, UserActivity, UserCountBreakdown, UserRole, UserSession, UserStats, UserStatus,
    UserWithSessions,
};
use crate::repository::{MergeResult, PurgeCount, UpdateUserParams, UserRepository};

#[derive(Clone)]
pub struct UserService {
    repository: Arc<UserRepository>,
    owner_reassignments: Arc<[DynOwnerReassignment]>,
}

impl fmt::Debug for UserService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let domains: Vec<&str> = self
            .owner_reassignments
            .iter()
            .map(|reassignment| reassignment.domain())
            .collect();
        f.debug_struct("UserService")
            .field("repository", &self.repository)
            .field("owner_reassignments", &domains)
            .finish()
    }
}

impl UserService {
    pub fn new(repository: Arc<UserRepository>) -> Self {
        Self {
            repository,
            owner_reassignments: Arc::from([]),
        }
    }

    #[must_use]
    pub fn with_owner_reassignments(mut self, reassignments: Vec<DynOwnerReassignment>) -> Self {
        self.owner_reassignments = Arc::from(reassignments);
        self
    }

    pub async fn find_by_id(&self, id: &UserId) -> Result<Option<User>> {
        self.repository.find_by_id(id).await
    }

    pub async fn find_by_email(&self, email: &str) -> Result<Option<User>> {
        self.repository.find_by_email(email).await
    }

    pub async fn find_by_name(&self, name: &str) -> Result<Option<User>> {
        self.repository.find_by_name(name).await
    }

    pub async fn find_by_role(&self, role: UserRole) -> Result<Vec<User>> {
        self.repository.find_by_role(role).await
    }

    pub async fn find_first_user(&self) -> Result<Option<User>> {
        self.repository.find_first_user().await
    }

    pub async fn find_first_admin(&self) -> Result<Option<User>> {
        self.repository.find_first_admin().await
    }

    pub async fn find_authenticated_user(&self, user_id: &UserId) -> Result<Option<User>> {
        self.repository.find_authenticated_user(user_id).await
    }

    pub async fn find_with_sessions(&self, user_id: &UserId) -> Result<Option<UserWithSessions>> {
        self.repository.find_with_sessions(user_id).await
    }

    pub async fn get_activity(&self, user_id: &UserId) -> Result<UserActivity> {
        self.repository.get_activity(user_id).await
    }

    pub async fn list(&self, limit: i64, offset: i64) -> Result<Vec<User>> {
        self.repository.list(limit, offset).await
    }

    pub async fn list_including_anonymous(&self, limit: i64, offset: i64) -> Result<Vec<User>> {
        self.repository
            .list_including_anonymous(limit, offset)
            .await
    }

    pub async fn list_all(&self) -> Result<Vec<User>> {
        self.repository.list_all().await
    }

    pub async fn search(&self, query: &str, limit: i64) -> Result<Vec<User>> {
        self.repository.search(query, limit).await
    }

    pub async fn search_including_anonymous(&self, query: &str, limit: i64) -> Result<Vec<User>> {
        self.repository
            .search_including_anonymous(query, limit)
            .await
    }

    pub async fn count(&self) -> Result<i64> {
        self.repository.count().await
    }

    pub async fn count_including_anonymous(&self) -> Result<i64> {
        self.repository.count_including_anonymous().await
    }

    pub async fn is_temporary_anonymous(&self, id: &UserId) -> Result<bool> {
        self.repository.is_temporary_anonymous(id).await
    }

    pub async fn list_non_anonymous_with_sessions(
        &self,
        limit: i64,
    ) -> Result<Vec<UserWithSessions>> {
        self.repository
            .list_non_anonymous_with_sessions(limit)
            .await
    }

    pub async fn list_sessions(&self, user_id: &UserId) -> Result<Vec<UserSession>> {
        self.repository.list_sessions(user_id).await
    }

    pub async fn list_active_sessions(&self, user_id: &UserId) -> Result<Vec<UserSession>> {
        self.repository.list_active_sessions(user_id).await
    }

    pub async fn list_recent_sessions(
        &self,
        user_id: &UserId,
        limit: i64,
    ) -> Result<Vec<UserSession>> {
        self.repository.list_recent_sessions(user_id, limit).await
    }

    pub async fn session_exists(&self, session_id: &SessionId) -> Result<bool> {
        self.repository.session_exists(session_id).await
    }

    pub async fn end_session(&self, session_id: &SessionId) -> Result<bool> {
        self.repository.end_session(session_id).await
    }

    pub async fn end_all_sessions(&self, user_id: &UserId) -> Result<u64> {
        self.repository.end_all_sessions(user_id).await
    }

    pub async fn create(
        &self,
        name: &str,
        email: &str,
        full_name: Option<&str>,
        display_name: Option<&str>,
    ) -> Result<User> {
        self.repository
            .create(name, email, full_name, display_name)
            .await
    }

    pub async fn create_if_absent(
        &self,
        name: &str,
        email: &str,
        full_name: Option<&str>,
        display_name: Option<&str>,
    ) -> Result<Option<User>> {
        self.repository
            .create_if_absent(name, email, full_name, display_name)
            .await
    }

    pub async fn create_anonymous(&self, fingerprint: &str) -> Result<User> {
        self.repository.create_anonymous(fingerprint).await
    }

    pub async fn find_or_create_federated(
        &self,
        issuer: &str,
        external_sub: &str,
        claims: &systemprompt_traits::FederatedIdentityClaims,
    ) -> Result<User> {
        self.repository
            .find_or_create_federated(issuer, external_sub, claims)
            .await
    }

    pub async fn update_email(&self, id: &UserId, email: &str) -> Result<User> {
        self.repository.update_email(id, email).await
    }

    pub async fn update_full_name(&self, id: &UserId, full_name: &str) -> Result<User> {
        self.repository.update_full_name(id, full_name).await
    }

    pub async fn update_status(&self, id: &UserId, status: UserStatus) -> Result<User> {
        self.repository.update_status(id, status).await
    }

    pub async fn update_email_verified(&self, id: &UserId, verified: bool) -> Result<User> {
        self.repository.update_email_verified(id, verified).await
    }

    pub async fn update_display_name(&self, id: &UserId, display_name: &str) -> Result<User> {
        self.repository.update_display_name(id, display_name).await
    }

    pub async fn update_all_fields(
        &self,
        id: &UserId,
        params: UpdateUserParams<'_>,
    ) -> Result<User> {
        self.repository.update_all_fields(id, params).await
    }

    pub async fn assign_roles(&self, id: &UserId, roles: &[String]) -> Result<User> {
        self.repository.assign_roles(id, roles).await
    }

    pub async fn delete(&self, id: &UserId) -> Result<Vec<PurgeCount>> {
        self.repository.delete(id).await
    }

    pub async fn purge_preview(&self, id: &UserId) -> Result<Vec<PurgeCount>> {
        self.repository.purge_preview(id).await
    }

    pub async fn cleanup_old_anonymous(&self, days: i32) -> Result<u64> {
        self.repository.cleanup_old_anonymous(days).await
    }

    pub async fn count_old_anonymous(&self, days: i32) -> Result<i64> {
        self.repository.count_old_anonymous(days).await
    }

    pub async fn count_with_breakdown(&self) -> Result<UserCountBreakdown> {
        let total = self.repository.count().await?;
        let by_status_vec = self.repository.count_by_status().await?;
        let by_role_vec = self.repository.count_by_role().await?;

        let by_status: HashMap<String, i64> = by_status_vec.into_iter().collect();
        let by_role: HashMap<String, i64> = by_role_vec.into_iter().collect();

        Ok(UserCountBreakdown {
            total,
            by_status,
            by_role,
        })
    }

    pub async fn get_stats(&self) -> Result<UserStats> {
        self.repository.get_stats().await
    }

    pub async fn list_by_filter(
        &self,
        status: Option<&str>,
        role: Option<&str>,
        older_than_days: Option<i64>,
        limit: i64,
    ) -> Result<Vec<User>> {
        self.repository
            .list_by_filter(status, role, older_than_days, limit)
            .await
    }

    pub async fn bulk_update_status(&self, user_ids: &[UserId], new_status: &str) -> Result<u64> {
        self.repository
            .bulk_update_status(user_ids, new_status)
            .await
    }

    pub async fn bulk_delete(&self, user_ids: &[UserId]) -> Result<u64> {
        self.repository.bulk_delete(user_ids).await
    }

    pub async fn merge_users(&self, source_id: &UserId, target_id: &UserId) -> Result<MergeResult> {
        if self.owner_reassignments.is_empty() {
            return Err(UserError::MergeUnavailable);
        }
        if source_id == target_id {
            return Err(UserError::Validation(
                "cannot merge a user into itself".to_owned(),
            ));
        }
        for id in [source_id, target_id] {
            if self.repository.find_by_id(id).await?.is_none() {
                return Err(UserError::NotFound(id.clone()));
            }
        }

        let mut tasks = 0;
        let mut total_rows = 0;
        for reassignment in self.owner_reassignments.iter() {
            let moved = reassignment
                .reassign_owner(source_id, target_id)
                .await
                .map_err(|source| UserError::OwnerReassignment {
                    domain: reassignment.domain(),
                    source,
                })?;
            tasks += moved
                .tables
                .iter()
                .filter(|(table, _)| *table == "agent_tasks")
                .map(|(_, rows)| rows)
                .sum::<u64>();
            total_rows += moved.total();
        }

        let sessions = self.repository.complete_merge(source_id, target_id).await?;
        Ok(MergeResult {
            sessions,
            tasks,
            total_rows: total_rows + sessions,
        })
    }

    pub async fn promote_anonymous(
        &self,
        source_id: &UserId,
        target_id: &UserId,
    ) -> Result<MergeResult> {
        if source_id == target_id {
            return Err(UserError::Validation(
                "cannot promote a user onto itself".to_owned(),
            ));
        }
        let source = self
            .repository
            .find_by_id(source_id)
            .await?
            .ok_or_else(|| UserError::NotFound(source_id.clone()))?;
        if !source.has_role(UserRole::Anonymous) {
            return Err(UserError::Validation(format!(
                "user {} is not anonymous; use an explicit admin merge instead",
                source_id
            )));
        }
        self.merge_users(source_id, target_id).await
    }
}
