//! Subject lookups for sharing: active users by name prefix and the roles
//! active users hold.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::Result;
use crate::models::{User, UserRole, UserRow, UserStatus};
use crate::repository::UserRepository;

const MAX_DIRECTORY_PAGE: i64 = 50;

fn like_prefix(query: &str) -> String {
    let escaped = query
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_");
    format!("{escaped}%")
}

impl UserRepository {
    pub async fn search_active_by_prefix(&self, query: &str, limit: i64) -> Result<Vec<User>> {
        let pattern = like_prefix(query.trim());
        let active = UserStatus::Active.as_str();
        let anonymous = UserRole::Anonymous.as_str();
        sqlx::query_as!(
            UserRow,
            r#"SELECT id, name, email, full_name, display_name, status, email_verified,
                      roles, avatar_url, is_bot, is_scanner, created_at, updated_at, account_kind
               FROM users
               WHERE status = $1 AND NOT ($2 = ANY(roles)) AND NOT is_bot
                 AND (display_name ILIKE $3 OR name ILIKE $3 OR email ILIKE $3
                      OR full_name ILIKE $3)
               ORDER BY COALESCE(display_name, full_name, name), id
               LIMIT $4"#,
            active,
            anonymous,
            pattern,
            limit.clamp(1, MAX_DIRECTORY_PAGE),
        )
        .fetch_all(&*self.pool)
        .await?
        .into_iter()
        .map(User::try_from)
        .collect()
    }

    pub async fn list_active_roles(&self) -> Result<Vec<String>> {
        let active = UserStatus::Active.as_str();
        let anonymous = UserRole::Anonymous.as_str();
        let roles = sqlx::query_scalar!(
            r#"SELECT DISTINCT role AS "role!" FROM users, UNNEST(roles) AS role
               WHERE status = $1 AND role <> $2 ORDER BY 1"#,
            active,
            anonymous,
        )
        .fetch_all(&*self.pool)
        .await?;
        Ok(roles)
    }
}
