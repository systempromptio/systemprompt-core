//! The user-backed [`SubjectDirectory`] the document store resolves grant
//! subjects through.
//!
//! A `user` subject names itself when the user is active, a `role` subject
//! names the active users holding the role, and a subject dimension names
//! the members the first extension audience resolver that handles it
//! returns; a dimension no resolver handles names nobody.
//!
//! The role catalogue is the built-in `admin` and `user` roles, the roles
//! extensions declare, and every role an active user holds.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_files::documents::{SubjectDirectory, SubjectDirectoryError, SubjectMatch};
use systemprompt_identifiers::UserId;
use systemprompt_models::auth::UserRole;
use systemprompt_models::notifications::AudienceSubject;
use systemprompt_users::{User, UserError, UserRepository};

use super::ExtensionAudiences;
use super::subjects::{ROLE_SUBJECT_KIND, USER_SUBJECT_KIND};

/// The [`SubjectDirectory`] over the user repository.
#[derive(Debug, Clone)]
pub struct UserSubjectDirectory {
    users: Arc<UserRepository>,
    catalog_roles: Vec<String>,
    audiences: ExtensionAudiences,
}

impl UserSubjectDirectory {
    #[must_use]
    pub const fn new(
        users: Arc<UserRepository>,
        catalog_roles: Vec<String>,
        audiences: ExtensionAudiences,
    ) -> Self {
        Self {
            users,
            catalog_roles,
            audiences,
        }
    }

    async fn dimension_members(
        &self,
        kind: &str,
        value: &str,
        limit: usize,
    ) -> Result<Vec<UserId>, SubjectDirectoryError> {
        let subject = AudienceSubject::Dimension {
            kind: kind.to_owned(),
            value: value.to_owned(),
        };
        let Some(resolver) = self
            .audiences
            .resolvers()
            .iter()
            .find(|resolver| resolver.handles(&subject))
        else {
            return Ok(Vec::new());
        };
        resolver
            .members(&subject, limit)
            .await
            .map_err(|error| SubjectDirectoryError(Box::new(error)))
    }
}

fn lookup_failed(error: UserError) -> SubjectDirectoryError {
    SubjectDirectoryError(Box::new(error))
}

#[async_trait]
impl SubjectDirectory for UserSubjectDirectory {
    async fn roles_of(&self, user: &UserId) -> Result<Option<Vec<String>>, SubjectDirectoryError> {
        let found = self.users.find_by_id(user).await.map_err(lookup_failed)?;
        Ok(found.filter(User::is_active).map(|u| u.roles))
    }

    async fn members(
        &self,
        kind: &str,
        value: &str,
        limit: usize,
    ) -> Result<Vec<UserId>, SubjectDirectoryError> {
        match kind {
            USER_SUBJECT_KIND => {
                let id = UserId::new(value);
                let found = self.users.find_by_id(&id).await.map_err(lookup_failed)?;
                Ok(found
                    .filter(User::is_active)
                    .map(|u| u.id)
                    .into_iter()
                    .collect())
            },
            ROLE_SUBJECT_KIND => self
                .users
                .list_active_ids_with_role(value, i64::try_from(limit).unwrap_or(i64::MAX))
                .await
                .map_err(lookup_failed),
            _ => self.dimension_members(kind, value, limit).await,
        }
    }

    async fn search_users(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<SubjectMatch>, SubjectDirectoryError> {
        let users = self
            .users
            .search_active_by_prefix(query, i64::try_from(limit).unwrap_or(i64::MAX))
            .await
            .map_err(lookup_failed)?;
        Ok(users
            .into_iter()
            .filter(|user| !user.is_service_account())
            .map(|user| SubjectMatch {
                value: user.id.as_str().to_owned(),
                detail: Some(user.email),
                display: user.display_name.or(user.full_name).unwrap_or(user.name),
            })
            .collect())
    }

    async fn is_person(&self, user: &UserId) -> Result<bool, SubjectDirectoryError> {
        let found = self.users.find_by_id(user).await.map_err(lookup_failed)?;
        Ok(found.is_some_and(|u| u.is_active() && !u.is_service_account()))
    }

    async fn roles(&self) -> Result<Vec<String>, SubjectDirectoryError> {
        let held = self
            .users
            .list_active_roles()
            .await
            .map_err(lookup_failed)?;
        let mut roles: Vec<String> = [UserRole::Admin, UserRole::User]
            .into_iter()
            .map(|role| role.as_str().to_owned())
            .chain(self.catalog_roles.iter().cloned())
            .chain(held)
            .collect();
        roles.sort();
        roles.dedup();
        Ok(roles)
    }
}
