//! The users domain's answer to "does anybody hold this role?" and "which
//! roles does this user hold?": the role directory governance's
//! `RoleDirectorySlot` holds, and the `role` subject-attribute provider the
//! gateway quota resolver reads for `subject: role` windows. Both read the
//! roles a user is granted on their `users` row.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use systemprompt_governance::authz::{AuthzError, RoleDirectory, SharedRoleDirectory};
use systemprompt_identifiers::UserId;
use systemprompt_traits::authz::{
    ROLE_PRECEDENCE, SubjectAttributeError, SubjectAttributeProvider, SubjectDimension,
};
use systemprompt_provider_contracts::governance::RuleType;

#[derive(Debug)]
pub struct UsersRoleDirectory {
    pool: Arc<PgPool>,
}

impl UsersRoleDirectory {
    #[must_use]
    pub fn shared(pool: Arc<PgPool>) -> SharedRoleDirectory {
        Arc::new(Self { pool })
    }
}

async fn roles_of(pool: &PgPool, user: &UserId) -> Result<Vec<String>, sqlx::Error> {
    let roles = sqlx::query_scalar!("SELECT roles FROM users WHERE id=$1", user.as_str())
        .fetch_optional(pool)
        .await?;
    Ok(roles.unwrap_or_default())
}

#[async_trait]
impl RoleDirectory for UsersRoleDirectory {
    async fn unknown_roles(&self, candidates: &[String]) -> Result<Vec<String>, AuthzError> {
        let rows = sqlx::query!(
            r#"
            SELECT candidate AS "candidate!"
            FROM UNNEST($1::text[]) AS candidate
            WHERE NOT EXISTS (
                SELECT 1 FROM users WHERE candidate = ANY(users.roles)
            )
            "#,
            candidates,
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(rows.into_iter().map(|row| row.candidate).collect())
    }

    async fn roles_of(&self, user: &UserId) -> Result<Vec<String>, AuthzError> {
        Ok(roles_of(&self.pool, user).await?)
    }
}

#[derive(Debug)]
pub struct UsersRoleSubjectProvider {
    pool: Arc<PgPool>,
}

impl UsersRoleSubjectProvider {
    #[must_use]
    pub const fn new(pool: Arc<PgPool>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SubjectAttributeProvider for UsersRoleSubjectProvider {
    fn dimension(&self) -> SubjectDimension {
        SubjectDimension {
            rule_type: RuleType::ROLE,
            label: "Role",
            precedence: ROLE_PRECEDENCE,
        }
    }

    async fn values_for(&self, user_id: &UserId) -> Result<Vec<String>, SubjectAttributeError> {
        roles_of(&self.pool, user_id)
            .await
            .map_err(|e| SubjectAttributeError::Lookup(Box::new(e)))
    }
}
