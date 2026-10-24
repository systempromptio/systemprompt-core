//! The users domain's answer to "does anybody hold this role?" and "which
//! roles does this user hold?", registered with the security crate at link time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use systemprompt_governance::authz::{AuthzError, RoleDirectory, SharedRoleDirectory};
use systemprompt_identifiers::UserId;

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
        let roles = sqlx::query_scalar!("SELECT roles FROM users WHERE id=$1", user.as_str())
            .fetch_optional(&*self.pool)
            .await?;
        Ok(roles.unwrap_or_default())
    }
}

systemprompt_governance::register_role_directory!(UsersRoleDirectory::shared);
