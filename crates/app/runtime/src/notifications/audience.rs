//! The core audience resolver over the user repository: a user subject is
//! the user when active, a role the active users holding it, and everyone
//! every active user. Dimension subjects belong to the
//! extension that registers the dimension.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::{Membership, NotificationAudienceError, NotificationAudienceResolver};
use systemprompt_identifiers::UserId;
use systemprompt_models::notifications::AudienceSubject;
use systemprompt_users::{User, UserError, UserRepository};

const NAME: &str = "core-users-roles";

/// The [`NotificationAudienceResolver`] for users, roles and everyone.
#[derive(Debug, Clone)]
pub struct UserRoleAudienceResolver {
    users: Arc<UserRepository>,
}

impl UserRoleAudienceResolver {
    #[must_use]
    pub const fn new(users: Arc<UserRepository>) -> Self {
        Self { users }
    }

    async fn active(&self, user: &UserId) -> Result<Membership, NotificationAudienceError> {
        let found = self.users.find_by_id(user).await.map_err(failed)?;
        Ok(membership(found.as_ref().is_some_and(User::is_active)))
    }
}

fn failed(error: UserError) -> NotificationAudienceError {
    NotificationAudienceError::Other {
        resolver: NAME,
        source: Box::new(error),
    }
}

#[async_trait]
impl NotificationAudienceResolver for UserRoleAudienceResolver {
    fn name(&self) -> &'static str {
        NAME
    }

    fn handles(&self, subject: &AudienceSubject) -> bool {
        !matches!(subject, AudienceSubject::Dimension { .. })
    }

    async fn members(
        &self,
        subject: &AudienceSubject,
        limit: usize,
    ) -> Result<Vec<UserId>, NotificationAudienceError> {
        match subject {
            AudienceSubject::User(user) => {
                let found = self.users.find_by_id(user).await.map_err(failed)?;
                Ok(found
                    .filter(User::is_active)
                    .map(|u| u.id)
                    .into_iter()
                    .collect())
            },
            AudienceSubject::Role(role) => self
                .users
                .list_active_ids_with_role(role, i64::try_from(limit).unwrap_or(i64::MAX))
                .await
                .map_err(failed),
            AudienceSubject::Everyone => self
                .users
                .list_active_ids(i64::try_from(limit).unwrap_or(i64::MAX))
                .await
                .map_err(failed),
            AudienceSubject::Dimension { .. } => Ok(Vec::new()),
        }
    }

    async fn is_member(
        &self,
        subject: &AudienceSubject,
        user: &UserId,
    ) -> Result<Membership, NotificationAudienceError> {
        match subject {
            AudienceSubject::User(member) if member != user => Ok(Membership::NotMember),
            AudienceSubject::User(_) | AudienceSubject::Everyone => self.active(user).await,
            AudienceSubject::Role(role) => {
                let held = self
                    .users
                    .is_active_with_role(user, role)
                    .await
                    .map_err(failed)?;
                Ok(membership(held))
            },
            AudienceSubject::Dimension { .. } => Ok(Membership::Unknown),
        }
    }
}

const fn membership(held: bool) -> Membership {
    if held {
        Membership::Member
    } else {
        Membership::NotMember
    }
}
