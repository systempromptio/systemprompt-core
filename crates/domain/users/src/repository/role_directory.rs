//! The users domain's answer to the authz ingestion's "does anybody hold
//! this role?" question, registered with the security crate at link time.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use systemprompt_security::authz::{AuthzError, RoleDirectory, SharedRoleDirectory};

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
}

systemprompt_security::register_role_directory!(UsersRoleDirectory::shared);
